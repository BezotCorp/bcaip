use crate::update_phase::UpdatePhase;
use crate::update_record::UpdateRecord;
use crate::update_state_store::UpdateStateStore;
use crate::watched_product::WatchedProduct;
use crate::watcher_request::WatcherRequest;
use crate::watcher_response::WatcherResponse;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use tungstenite::{Message, WebSocket, accept};
use uuid::{Uuid, Version};

pub(crate) fn handle_connection(
    stream: TcpStream,
    products: &HashMap<String, WatchedProduct>,
    state_store: &UpdateStateStore,
    self_product: Option<&str>,
) -> Result<bool, String> {
    let mut socket = accept(stream)
        .map_err(|error| format!("WebSocket handshake failed: {error}"))?;

    let message = socket
        .read()
        .map_err(|error| format!("failed to read watcher request: {error}"))?;

    let Message::Text(text) = message else {
        let _ = send_response(
            &mut socket,
            &WatcherResponse::Failure {
                message: "first watcher message must be a RON text request".to_string(),
            },
        );

        return Ok(false);
    };

    let request: WatcherRequest = ron::from_str(text.as_str())
        .map_err(|error| format!("invalid watcher request: {error}"))?;

    match request {
        WatcherRequest::Status { product } => {
            let record = state_store.read(&product)?;

            send_response(
                &mut socket,
                &WatcherResponse::Status { record },
            )?;

            Ok(false)
        }

        WatcherRequest::Begin {
            update_id,
            product,
            expected_size,
            sha256,
        } => {
            let watched_product = products
                .get(&product)
                .ok_or_else(|| format!("unknown product: {product}"))?;

            reconcile_pending(watched_product, state_store)?;

            validate_update_id(&update_id)?;
            validate_sha256(&sha256)?;

            if expected_size == 0 {
                return Err("expected_size must be greater than zero".to_string());
            }

            if let Some(existing) = state_store.read(&product)? {
                if existing.update_id == update_id
                    && existing.phase == UpdatePhase::Committed
                {
                    send_response(
                        &mut socket,
                        &WatcherResponse::Success {
                            update_id,
                            product: product.clone(),
                        },
                    )?;

                    return Ok(self_product == Some(product.as_str()));
                }
            }

            receive_update(
                &mut socket,
                watched_product,
                state_store,
                update_id,
                expected_size,
                sha256.to_ascii_lowercase(),
            )?;

            Ok(self_product == Some(product.as_str()))
        }
    }
}

pub(crate) fn reconcile_pending(
    product: &WatchedProduct,
    state_store: &UpdateStateStore,
) -> Result<(), String> {
    let Some(mut record) = state_store.read(&product.name)? else {
        return Ok(());
    };

    if record.phase != UpdatePhase::Pending {
        return Ok(());
    }

    let target = product.executable_path();
    let temp = temporary_update_path(&target, &record.update_id)?;

    if file_matches(&target, record.expected_size, &record.sha256)? {
        record.phase = UpdatePhase::Committed;
        record.received_size = record.expected_size;
        record.error = None;

        state_store.write(&record)?;

        let _ = fs::remove_file(temp);

        return Ok(());
    }

    let _ = fs::remove_file(temp);

    record.phase = UpdatePhase::Failed;
    record.error = Some(
        "watcher restarted or transfer ended before the update committed".to_string(),
    );

    state_store.write(&record)
}

fn receive_update(
    socket: &mut WebSocket<TcpStream>,
    product: &WatchedProduct,
    state_store: &UpdateStateStore,
    update_id: String,
    expected_size: u64,
    sha256: String,
) -> Result<(), String> {
    let target = product.executable_path();

    if !target.is_file() {
        return Err(format!(
            "{} executable does not exist: {}",
            product.name,
            target.display()
        ));
    }

    let parent = target.parent().ok_or_else(|| {
        format!("{} has no parent directory", target.display())
    })?;

    let permissions = fs::metadata(&target)
        .map_err(|error| {
            format!("failed to read {} metadata: {error}", target.display())
        })?
        .permissions();

    let temp = temporary_update_path(&target, &update_id)?;

    if temp.exists() {
        fs::remove_file(&temp).map_err(|error| {
            format!("failed to remove stale {}: {error}", temp.display())
        })?;
    }

    let mut record = UpdateRecord {
        update_id: update_id.clone(),
        product: product.name.clone(),
        phase: UpdatePhase::Pending,
        expected_size,
        received_size: 0,
        sha256: sha256.clone(),
        error: None,
    };

    state_store.write(&record)?;

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|error| {
            format!("failed to create {}: {error}", temp.display())
        })?;

    if let Err(error) = send_response(
        socket,
        &WatcherResponse::Accepted {
            update_id: update_id.clone(),
            product: product.name.clone(),
        },
    ) {
        let _ = fs::remove_file(&temp);

        record.phase = UpdatePhase::Failed;
        record.error = Some(error.clone());
        let _ = state_store.write(&record);

        return Err(error);
    }

    let mut hasher = Sha256::new();

    let transfer_result = (|| -> Result<(), String> {
        while record.received_size < expected_size {
            let message = socket
                .read()
                .map_err(|error| format!("update transfer interrupted: {error}"))?;

            match message {
                Message::Binary(payload) => {
                    let payload_size = payload.len() as u64;

                    let next_size = record
                        .received_size
                        .checked_add(payload_size)
                        .ok_or_else(|| "received size overflow".to_string())?;

                    if next_size > expected_size {
                        return Err(format!(
                            "received more bytes than expected: {} > {}",
                            next_size,
                            expected_size
                        ));
                    }

                    file.write_all(payload.as_ref()).map_err(|error| {
                        format!("failed to write {}: {error}", temp.display())
                    })?;

                    hasher.update(payload.as_ref());
                    record.received_size = next_size;
                }

                Message::Ping(payload) => {
                    socket
                        .send(Message::Pong(payload))
                        .map_err(|error| format!("failed to send Pong: {error}"))?;
                }

                Message::Close(_) => {
                    return Err(format!(
                        "connection closed after {} of {} bytes",
                        record.received_size,
                        expected_size
                    ));
                }

                _ => {
                    return Err(
                        "only binary frames are accepted during an update transfer"
                            .to_string(),
                    );
                }
            }
        }

        file.flush().map_err(|error| {
            format!("failed to flush {}: {error}", temp.display())
        })?;

        file.sync_all().map_err(|error| {
            format!("failed to sync {}: {error}", temp.display())
        })?;

        let digest = hasher.finalize();
        let actual_sha256 = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();

        if actual_sha256 != sha256 {
            return Err(format!(
                "SHA-256 mismatch: expected {sha256}, got {actual_sha256}"
            ));
        }

        fs::set_permissions(&temp, permissions).map_err(|error| {
            format!(
                "failed to preserve executable permissions on {}: {error}",
                temp.display()
            )
        })?;

        fs::rename(&temp, &target).map_err(|error| {
            format!(
                "failed to atomically replace {}: {error}",
                target.display()
            )
        })?;

        sync_directory(parent)?;

        Ok(())
    })();

    if let Err(error) = transfer_result {
        let binary_was_committed =
            file_matches(&target, expected_size, &sha256).unwrap_or(false);

        if binary_was_committed {
            record.received_size = expected_size;

            if state_store.write(&record).is_err() {
                let _ = socket.send(Message::Text(
                    ron::ser::to_string(&WatcherResponse::Failure {
                        message: format!(
                            "binary committed but watcher state is still pending: {error}"
                        ),
                    })
                    .unwrap_or_else(|_| "Failure".to_string())
                    .into(),
                ));

                return Ok(());
            }
        } else {
            let _ = fs::remove_file(&temp);

            record.phase = UpdatePhase::Failed;
            record.error = Some(error.clone());

            let _ = state_store.write(&record);

            let _ = send_response(
                socket,
                &WatcherResponse::Failure {
                    message: error,
                },
            );

            return Ok(());
        }
    }

    record.phase = UpdatePhase::Committed;
    record.received_size = expected_size;
    record.error = None;

    state_store.write(&record)?;

    send_response(
        socket,
        &WatcherResponse::Success {
            update_id,
            product: product.name.clone(),
        },
    )?;

    let _ = socket.close(None);

    Ok(())
}

fn send_response(
    socket: &mut WebSocket<TcpStream>,
    response: &WatcherResponse,
) -> Result<(), String> {
    let serialized = ron::ser::to_string(response)
        .map_err(|error| format!("failed to encode watcher response: {error}"))?;

    socket
        .send(Message::Text(serialized.into()))
        .map_err(|error| format!("failed to send watcher response: {error}"))
}

fn validate_update_id(value: &str) -> Result<(), String> {
    let uuid = Uuid::parse_str(value)
        .map_err(|error| format!("invalid update UUID: {error}"))?;

    if uuid.get_version() != Some(Version::SortRand) {
        return Err(format!("update_id must be UUIDv7: {uuid}"));
    }

    Ok(())
}

fn validate_sha256(value: &str) -> Result<(), String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("sha256 must contain exactly 64 hexadecimal characters".to_string());
    }

    Ok(())
}

fn temporary_update_path(
    target: &Path,
    update_id: &str,
) -> Result<PathBuf, String> {
    let parent = target.parent().ok_or_else(|| {
        format!("{} has no parent directory", target.display())
    })?;

    let file_name = target
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("invalid executable filename: {}", target.display()))?;

    Ok(parent.join(format!(
        ".{file_name}.{update_id}.update.tmp"
    )))
}

fn file_matches(
    path: &Path,
    expected_size: u64,
    expected_sha256: &str,
) -> Result<bool, String> {
    if !path.is_file() {
        return Ok(false);
    }

    let mut file = fs::File::open(path)
        .map_err(|error| format!("failed to open {}: {error}", path.display()))?;

    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut size = 0_u64;

    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

        if read == 0 {
            break;
        }

        size = size
            .checked_add(read as u64)
            .ok_or_else(|| "file size overflow".to_string())?;

        hasher.update(&buffer[..read]);
    }

    let digest = hasher.finalize();
    let sha256 = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();

    Ok(
        size == expected_size
            && sha256.eq_ignore_ascii_case(expected_sha256),
    )
}

fn sync_directory(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            format!(
                "failed to sync directory {}: {error}",
                path.display()
            )
        })
}

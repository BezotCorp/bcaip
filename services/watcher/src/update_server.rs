use crate::begin_update_result::BeginUpdateResult;
use crate::executable::Executable;
use crate::executable_store::ExecutableStore;
use crate::process::Process;
use crate::update::Update;
use crate::update_phase::UpdatePhase;
use crate::watcher_request::WatcherRequest;
use crate::watcher_response::WatcherResponse;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use tungstenite::{Message, WebSocket, accept};
use uuid::{Uuid, Version};

pub(crate) fn handle_connection(
    stream: TcpStream,
    store: &ExecutableStore,
    watcher_name: &str,
    watcher_instance_id: &str,
    watcher_applied_update_id: Option<&str>,
) -> Result<(), String> {
    let mut socket =
        accept(stream).map_err(|error| format!("WebSocket handshake failed: {error}"))?;

    let message = socket
        .read()
        .map_err(|error| format!("failed to read watcher request: {error}"))?;

    let text = match message {
        Message::Ping(payload) => {
            socket
                .send(Message::Pong(payload))
                .map_err(|error| format!("failed to send watcher Pong: {error}"))?;

            return Ok(());
        }

        Message::Text(text) => text,

        _ => {
            send_failure(&mut socket, "first watcher message must be RON text")?;

            return Ok(());
        }
    };

    let request: WatcherRequest = ron::from_str(text.as_str())
        .map_err(|error| format!("invalid watcher request: {error}"))?;

    match request {
        WatcherRequest::Health => send_response(
            &mut socket,
            &WatcherResponse::Healthy {
                executable: watcher_name.to_string(),
                instance_id: watcher_instance_id.to_string(),
                applied_update_id: watcher_applied_update_id.map(|value| value.to_string()),
            },
        ),

        WatcherRequest::ExpectProcess {
            executable,
            instance_id,
        } => {
            validate_instance_id(&instance_id)?;

            if store.get(&executable)?.is_none() {
                send_failure(&mut socket, "unknown executable")?;

                return Ok(());
            }

            store.expect_process(&executable, &instance_id)?;

            send_response(&mut socket, &WatcherResponse::Acknowledged)
        }

        WatcherRequest::Status { executable } => {
            let Some(executable) = store.get(&executable)? else {
                send_failure(&mut socket, "unknown executable")?;

                return Ok(());
            };

            send_response(&mut socket, &WatcherResponse::Status { executable })
        }

        WatcherRequest::ProcessStarted {
            executable,
            pid,
            instance_id,
            ws_address,
            applied_update_id,
        } => {
            validate_instance_id(&instance_id)?;

            if store.get(&executable)?.is_none() {
                send_failure(&mut socket, "unknown executable")?;

                return Ok(());
            }

            store.set_process(
                &executable,
                Process {
                    pid,
                    instance_id,
                    ws_address,
                    applied_update_id,
                },
            )?;

            send_response(&mut socket, &WatcherResponse::Acknowledged)
        }

        WatcherRequest::ProcessStopped {
            executable,
            instance_id,
        } => {
            validate_instance_id(&instance_id)?;

            if store.get(&executable)?.is_none() {
                send_failure(&mut socket, "unknown executable")?;

                return Ok(());
            }

            store.clear_process(&executable, &instance_id)?;

            send_response(&mut socket, &WatcherResponse::Acknowledged)
        }

        WatcherRequest::BeginUpdate {
            update_id,
            executable,
            expected_size,
            sha256,
        } => {
            let Some(definition) = store.get(&executable)? else {
                send_failure(&mut socket, "unknown executable")?;

                return Ok(());
            };

            validate_update_id(&update_id)?;
            validate_sha256(&sha256)?;

            if expected_size == 0 {
                send_failure(&mut socket, "expected_size must be greater than zero")?;

                return Ok(());
            }

            let sha256 = sha256.to_ascii_lowercase();

            let begin_update = match store.begin_update(
                &executable,
                update_id.clone(),
                expected_size,
                sha256.clone(),
            ) {
                Ok(result) => result,

                Err(error) => {
                    send_failure(&mut socket, &error)?;

                    return Ok(());
                }
            };

            match begin_update {
                BeginUpdateResult::AlreadyCommitted => send_response(
                    &mut socket,
                    &WatcherResponse::Success {
                        update_id,
                        executable,
                    },
                ),

                BeginUpdateResult::AlreadyPending => {
                    send_failure(&mut socket, "the update is already being transferred")
                }

                BeginUpdateResult::Started => receive_update(
                    &mut socket,
                    &definition,
                    store,
                    update_id,
                    expected_size,
                    sha256,
                ),
            }
        }
    }
}

pub(crate) fn reconcile_all(store: &ExecutableStore) -> Result<(), String> {
    for executable in store.snapshot()? {
        reconcile_pending(&executable, store)?;
    }

    Ok(())
}

fn reconcile_pending(executable: &Executable, store: &ExecutableStore) -> Result<(), String> {
    let Some(mut update) = store.get(&executable.name)?.and_then(|value| value.update) else {
        return Ok(());
    };

    if update.phase != UpdatePhase::Pending {
        return Ok(());
    }

    let target = executable.path();
    let temp = temporary_update_path(&target, &update.update_id)?;

    if file_matches(&target, update.expected_size, &update.sha256)? {
        let parent = target
            .parent()
            .ok_or_else(|| format!("{} has no parent directory", target.display()))?;

        sync_directory(parent)?;

        update.phase = UpdatePhase::Committed;
        update.received_size = update.expected_size;
        update.error = None;

        store.set_update(&executable.name, update)?;

        let _ = fs::remove_file(temp);

        return Ok(());
    }

    let _ = fs::remove_file(temp);

    update.phase = UpdatePhase::Failed;
    update.error = Some("watcher restarted before the update committed".to_string());

    store.set_update(&executable.name, update)
}

fn receive_update(
    socket: &mut WebSocket<TcpStream>,
    executable: &Executable,
    store: &ExecutableStore,
    update_id: String,
    expected_size: u64,
    sha256: String,
) -> Result<(), String> {
    let mut update = Update {
        update_id: update_id.clone(),
        phase: UpdatePhase::Pending,
        expected_size,
        received_size: 0,
        sha256: sha256.clone(),
        error: None,
    };

    let target = executable.path();

    if !target.is_file() {
        return fail_reserved_update(
            socket,
            executable,
            store,
            update,
            format!("executable does not exist: {}", target.display()),
        );
    }

    let parent = match target.parent() {
        Some(parent) => parent,

        None => {
            return fail_reserved_update(
                socket,
                executable,
                store,
                update,
                format!("{} has no parent directory", target.display()),
            );
        }
    };

    let permissions = match fs::metadata(&target) {
        Ok(metadata) => metadata.permissions(),

        Err(error) => {
            return fail_reserved_update(
                socket,
                executable,
                store,
                update,
                format!("failed to read {} metadata: {error}", target.display()),
            );
        }
    };

    let temp = match temporary_update_path(&target, &update_id) {
        Ok(temp) => temp,

        Err(error) => {
            return fail_reserved_update(socket, executable, store, update, error);
        }
    };

    if temp.exists() {
        if let Err(error) = fs::remove_file(&temp) {
            return fail_reserved_update(
                socket,
                executable,
                store,
                update,
                format!("failed to remove stale {}: {error}", temp.display()),
            );
        }
    }

    let mut file = match OpenOptions::new().write(true).create_new(true).open(&temp) {
        Ok(file) => file,

        Err(error) => {
            return fail_reserved_update(
                socket,
                executable,
                store,
                update,
                format!("failed to create {}: {error}", temp.display()),
            );
        }
    };

    if let Err(error) = send_response(
        socket,
        &WatcherResponse::Accepted {
            update_id: update_id.clone(),
            executable: executable.name.clone(),
        },
    ) {
        let _ = fs::remove_file(&temp);

        update.phase = UpdatePhase::Failed;
        update.error = Some(error.clone());

        let _ = store.set_update(&executable.name, update);

        return Err(error);
    }

    let mut hasher = Sha256::new();

    while update.received_size < expected_size {
        let message = match socket.read() {
            Ok(message) => message,

            Err(error) => {
                return fail_transfer(
                    socket,
                    executable,
                    store,
                    update,
                    &temp,
                    format!("update transfer interrupted: {error}"),
                );
            }
        };

        match message {
            Message::Binary(payload) => {
                let payload_size = payload.len() as u64;

                let Some(next_size) = update.received_size.checked_add(payload_size) else {
                    return fail_transfer(
                        socket,
                        executable,
                        store,
                        update,
                        &temp,
                        "received size overflow".to_string(),
                    );
                };

                if next_size > expected_size {
                    return fail_transfer(
                        socket,
                        executable,
                        store,
                        update,
                        &temp,
                        format!(
                            "received more bytes than expected: {} > {}",
                            next_size, expected_size
                        ),
                    );
                }

                if let Err(error) = file.write_all(payload.as_ref()) {
                    return fail_transfer(
                        socket,
                        executable,
                        store,
                        update,
                        &temp,
                        format!("failed to write {}: {error}", temp.display()),
                    );
                }

                hasher.update(payload.as_ref());

                update.received_size = next_size;
            }

            Message::Ping(payload) => {
                if let Err(error) = socket.send(Message::Pong(payload)) {
                    return fail_transfer(
                        socket,
                        executable,
                        store,
                        update,
                        &temp,
                        format!("failed to send Pong: {error}"),
                    );
                }
            }

            Message::Close(_) => {
                let received_size = update.received_size;

                return fail_transfer(
                    socket,
                    executable,
                    store,
                    update,
                    &temp,
                    format!(
                        "connection closed after {} of {} bytes",
                        received_size, expected_size
                    ),
                );
            }

            _ => {
                return fail_transfer(
                    socket,
                    executable,
                    store,
                    update,
                    &temp,
                    "only binary frames are accepted during update transfer".to_string(),
                );
            }
        }
    }

    if let Err(error) = file.flush() {
        return fail_transfer(
            socket,
            executable,
            store,
            update,
            &temp,
            format!("failed to flush {}: {error}", temp.display()),
        );
    }

    if let Err(error) = file.sync_all() {
        return fail_transfer(
            socket,
            executable,
            store,
            update,
            &temp,
            format!("failed to sync {}: {error}", temp.display()),
        );
    }

    let digest = hasher.finalize();

    let actual_sha256 = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();

    if actual_sha256 != sha256 {
        return fail_transfer(
            socket,
            executable,
            store,
            update,
            &temp,
            format!(
                "SHA-256 mismatch: expected {}, got {}",
                sha256, actual_sha256
            ),
        );
    }

    if let Err(error) = fs::set_permissions(&temp, permissions) {
        return fail_transfer(
            socket,
            executable,
            store,
            update,
            &temp,
            format!(
                "failed to preserve executable permissions on {}: {error}",
                temp.display()
            ),
        );
    }

    if let Err(error) = fs::rename(&temp, &target) {
        return fail_transfer(
            socket,
            executable,
            store,
            update,
            &temp,
            format!("failed to atomically replace {}: {error}", target.display()),
        );
    }

    /*
     * Bytes have been renamed into place, but durability is not
     * confirmed until the parent directory is synced.
     *
     * If this fails we deliberately KEEP Pending. Startup
     * reconciliation hashes the target and retries the directory sync.
     */
    if let Err(error) = sync_directory(parent) {
        update.error = Some(error.clone());

        let _ = store.set_update(&executable.name, update);

        send_failure(socket, &error)?;

        return Ok(());
    }

    update.phase = UpdatePhase::Committed;
    update.received_size = expected_size;
    update.error = None;

    if let Err(error) = store.set_update(&executable.name, update) {
        send_failure(
            socket,
            &format!("binary committed but committed state could not be persisted: {error}"),
        )?;

        return Ok(());
    }

    send_response(
        socket,
        &WatcherResponse::Success {
            update_id,
            executable: executable.name.clone(),
        },
    )?;

    let _ = socket.close(None);

    Ok(())
}

fn fail_reserved_update(
    socket: &mut WebSocket<TcpStream>,
    executable: &Executable,
    store: &ExecutableStore,
    mut update: Update,
    error: String,
) -> Result<(), String> {
    update.phase = UpdatePhase::Failed;
    update.error = Some(error.clone());

    let _ = store.set_update(&executable.name, update);

    let _ = send_failure(socket, &error);

    Ok(())
}

fn fail_transfer(
    socket: &mut WebSocket<TcpStream>,
    executable: &Executable,
    store: &ExecutableStore,
    mut update: Update,
    temp: &Path,
    error: String,
) -> Result<(), String> {
    let _ = fs::remove_file(temp);

    update.phase = UpdatePhase::Failed;
    update.error = Some(error.clone());

    let _ = store.set_update(&executable.name, update);

    let _ = send_failure(socket, &error);

    Ok(())
}

fn send_failure(socket: &mut WebSocket<TcpStream>, message: &str) -> Result<(), String> {
    send_response(
        socket,
        &WatcherResponse::Failure {
            message: message.to_string(),
        },
    )
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
    let uuid = Uuid::parse_str(value).map_err(|error| format!("invalid update UUID: {error}"))?;

    if uuid.get_version() != Some(Version::SortRand) {
        return Err(format!("update_id must be UUIDv7: {uuid}"));
    }

    Ok(())
}

fn validate_instance_id(value: &str) -> Result<(), String> {
    let uuid = Uuid::parse_str(value).map_err(|error| format!("invalid instance UUID: {error}"))?;

    if uuid.get_version() != Some(Version::SortRand) {
        return Err(format!("instance_id must be UUIDv7: {uuid}"));
    }

    Ok(())
}

fn validate_sha256(value: &str) -> Result<(), String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("sha256 must contain exactly 64 hexadecimal characters".to_string());
    }

    Ok(())
}

fn temporary_update_path(target: &Path, update_id: &str) -> Result<PathBuf, String> {
    let parent = target
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", target.display()))?;

    let file_name = target
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("invalid executable filename: {}", target.display()))?;

    Ok(parent.join(format!(".{file_name}.{update_id}.update.tmp")))
}

fn file_matches(path: &Path, expected_size: u64, expected_sha256: &str) -> Result<bool, String> {
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

    let actual_sha256 = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();

    Ok(size == expected_size && actual_sha256.eq_ignore_ascii_case(expected_sha256))
}

fn sync_directory(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("failed to sync directory {}: {error}", path.display()))
}

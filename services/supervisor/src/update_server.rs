use std::fs::{self, OpenOptions};
use std::io::Write;
use std::net::SocketAddr;
use std::path::Path;

use crate::executable_backup::ExecutableBackup;
use crate::executable_store::ExecutableStore;
use crate::handover_journal::HandoverJournal;
use crate::handover_phase::HandoverPhase;
use crate::supervisor_config::SupervisorConfig;
use crate::systemd_notifier::SystemdNotifier;
use executables_manager::BeginUpdateResult;
use executables_manager::CandidateReport;
use executables_manager::Executable;
use executables_manager::UpdateFile;
use executables_manager::{
    ExecutableManagementRequest, ExecutableManagementResponse, Process, Update,
};
use sha2::{Digest, Sha256};
use uuid::{Uuid, Version};
use ws_manager::{WsConnection, WsMessage};

pub(crate) struct UpdateServer<'a> {
    store: &'a ExecutableStore,
    config: &'a SupervisorConfig,
    supervisor_instance_id: &'a str,
    supervisor_applied_update_id: Option<&'a str>,
}

impl<'a> UpdateServer<'a> {
    pub(crate) fn new(
        store: &'a ExecutableStore,
        config: &'a SupervisorConfig,
        supervisor_instance_id: &'a str,
        supervisor_applied_update_id: Option<&'a str>,
    ) -> Self {
        Self {
            store,
            config,
            supervisor_instance_id,
            supervisor_applied_update_id,
        }
    }

    pub(crate) fn handle_connection(&self, mut connection: WsConnection) -> Result<(), String> {
        let message = connection
            .read()
            .map_err(|error| format!("failed to read executable management request: {error}"))?;

        let text = match message {
            WsMessage::Ping(payload) => {
                connection
                    .send_pong(&payload)
                    .map_err(|error| format!("failed to send supervisor Pong: {error}"))?;

                return Ok(());
            }

            WsMessage::Text(text) => text,

            _ => {
                Self::send_failure(&mut connection, "first supervisor message must be RON text")?;

                return Ok(());
            }
        };

        let request: ExecutableManagementRequest = ron::from_str(&text)
            .map_err(|error| format!("invalid executable management request: {error}"))?;

        self.handle_request(&mut connection, request)
    }

    pub(crate) fn reconcile_all(
        store: &ExecutableStore,
        executable_root: &Path,
    ) -> Result<(), String> {
        for executable in store.snapshot()? {
            Self::reconcile_pending(&executable, store, executable_root)?;
        }

        Ok(())
    }

    fn handle_request(
        &self,
        socket: &mut WsConnection,
        request: ExecutableManagementRequest,
    ) -> Result<(), String> {
        match request {
            ExecutableManagementRequest::Health => Self::send_response(
                socket,
                &ExecutableManagementResponse::Healthy {
                    executable: self.config.supervisor_name().to_string(),
                    instance_id: self.supervisor_instance_id.to_string(),
                    applied_update_id: self
                        .supervisor_applied_update_id
                        .map(|value| value.to_string()),
                },
            ),

            ExecutableManagementRequest::CandidateReport => {
                let report = CandidateReport::new(
                    self.config.supervisor_name().to_string(),
                    self.supervisor_instance_id.to_string(),
                    self.supervisor_applied_update_id.map(str::to_string),
                    self.store.can_write(),
                    self.store.runtime_fingerprint()?,
                );

                Self::send_response(socket, &ExecutableManagementResponse::Candidate(report))
            }

            ExecutableManagementRequest::PromoteCandidate {
                executable,
                instance_id,
            } => {
                Self::validate_instance_id(&instance_id)?;

                if executable != self.config.supervisor_name() {
                    Self::send_failure(socket, "candidate promotion executable mismatch")?;

                    return Ok(());
                }

                if instance_id != self.supervisor_instance_id {
                    Self::send_failure(socket, "candidate promotion instance mismatch")?;

                    return Ok(());
                }

                let Some(address) = self.config.promoted_address() else {
                    Self::send_failure(socket, "only a supervisor candidate can be promoted")?;

                    return Ok(());
                };

                match self.promote(address) {
                    Ok(()) => Self::send_response(socket, &ExecutableManagementResponse::Promoted),
                    Err(error) => Self::send_failure(socket, &error),
                }
            }

            ExecutableManagementRequest::ExpectProcess {
                executable,
                instance_id,
            } => {
                Self::validate_instance_id(&instance_id)?;

                if self.store.get(&executable)?.is_none() {
                    Self::send_failure(socket, "unknown executable")?;

                    return Ok(());
                }

                self.store.expect_process(&executable, &instance_id)?;

                Self::send_response(socket, &ExecutableManagementResponse::Acknowledged)
            }

            ExecutableManagementRequest::Status { executable } => {
                let Some(executable) = self.store.get(&executable)? else {
                    Self::send_failure(socket, "unknown executable")?;

                    return Ok(());
                };

                Self::send_response(socket, &ExecutableManagementResponse::Status { executable })
            }

            ExecutableManagementRequest::ProcessStarted {
                executable,
                pid,
                instance_id,
                ws_address,
                applied_update_id,
            } => {
                Self::validate_instance_id(&instance_id)?;

                if self.store.get(&executable)?.is_none() {
                    Self::send_failure(socket, "unknown executable")?;

                    return Ok(());
                }

                self.store.set_process(
                    &executable,
                    Process::new(pid, instance_id, ws_address, applied_update_id),
                )?;

                Self::send_response(socket, &ExecutableManagementResponse::Acknowledged)
            }

            ExecutableManagementRequest::ProcessStopped {
                executable,
                instance_id,
            } => {
                Self::validate_instance_id(&instance_id)?;

                if self.store.get(&executable)?.is_none() {
                    Self::send_failure(socket, "unknown executable")?;

                    return Ok(());
                }

                self.store.clear_process(&executable, &instance_id)?;

                Self::send_response(socket, &ExecutableManagementResponse::Acknowledged)
            }

            ExecutableManagementRequest::BeginUpdate {
                update_id,
                executable,
                expected_size,
                sha256,
            } => {
                let Some(definition) = self.store.get(&executable)? else {
                    Self::send_failure(socket, "unknown executable")?;

                    return Ok(());
                };

                Self::validate_update_id(&update_id)?;
                Self::validate_sha256(&sha256)?;

                if expected_size == 0 {
                    Self::send_failure(socket, "expected_size must be greater than zero")?;

                    return Ok(());
                }

                let sha256 = sha256.to_ascii_lowercase();

                let begin_update = match self.store.begin_update(
                    &executable,
                    update_id.clone(),
                    expected_size,
                    sha256.clone(),
                ) {
                    Ok(result) => result,

                    Err(error) => {
                        Self::send_failure(socket, &error)?;

                        return Ok(());
                    }
                };

                match begin_update {
                    BeginUpdateResult::AlreadyCommitted => Self::send_response(
                        socket,
                        &ExecutableManagementResponse::Success {
                            update_id,
                            executable,
                        },
                    ),

                    BeginUpdateResult::AlreadyPending => {
                        Self::send_failure(socket, "the update is already being transferred")
                    }

                    BeginUpdateResult::Started => {
                        self.receive_update(socket, &definition, update_id, expected_size, sha256)
                    }
                }
            }
        }
    }

    /// Commit point of a handover, in this order: the authority and runtime
    /// state, then systemd's main process, then the journal. Any failure
    /// before the journal commit gives the authority back; the caller rolls
    /// the whole transaction back.
    fn promote(&self, address: SocketAddr) -> Result<(), String> {
        let name = self.config.supervisor_name();
        let mut journal = HandoverJournal::load(self.config.data_directory())?
            .filter(|journal| journal.authorizes_promotion(name, self.supervisor_applied_update_id))
            .ok_or_else(|| "no handover is waiting for this candidate".to_string())?;

        self.store.promote_candidate(
            name,
            Process::new(
                std::process::id(),
                self.supervisor_instance_id.to_string(),
                address,
                self.supervisor_applied_update_id.map(str::to_string),
            ),
        )?;

        SystemdNotifier::main_pid(std::process::id())
            .and_then(|()| journal.advance(HandoverPhase::Committed))
            .inspect_err(|_| self.store.release_authority())
    }

    fn reconcile_pending(
        executable: &Executable,
        store: &ExecutableStore,
        executable_root: &Path,
    ) -> Result<(), String> {
        let Some(mut update) = store
            .get(executable.name().as_str())?
            .and_then(|value| value.update_owned())
        else {
            return Ok(());
        };

        if !update.is_pending() {
            return Ok(());
        }

        let target = executable.path(executable_root);
        let temp = UpdateFile::temporary_path(&target, update.update_id())?;

        if UpdateFile::matches(&target, update.expected_size(), update.sha256())? {
            let parent = target
                .parent()
                .ok_or_else(|| format!("{} has no parent directory", target.display()))?;

            UpdateFile::sync_directory(parent)?;

            update.commit();

            store.set_update(executable.name().as_str(), update)?;

            let _ = fs::remove_file(temp);

            return Ok(());
        }

        let _ = fs::remove_file(temp);

        update.fail("supervisor restarted before the update committed".to_string());

        store.set_update(executable.name().as_str(), update)
    }

    fn receive_update(
        &self,
        socket: &mut WsConnection,
        executable: &Executable,
        update_id: String,
        expected_size: u64,
        sha256: String,
    ) -> Result<(), String> {
        let mut update = Update::new(update_id.clone(), expected_size, sha256.clone());

        let target = executable.path(self.config.executable_root());

        if !target.is_file() {
            return self.fail_reserved_update(
                socket,
                executable,
                update,
                format!("executable does not exist: {}", target.display()),
            );
        }

        let parent = match target.parent() {
            Some(parent) => parent,

            None => {
                return self.fail_reserved_update(
                    socket,
                    executable,
                    update,
                    format!("{} has no parent directory", target.display()),
                );
            }
        };

        let permissions = match fs::metadata(&target) {
            Ok(metadata) => metadata.permissions(),

            Err(error) => {
                return self.fail_reserved_update(
                    socket,
                    executable,
                    update,
                    format!("failed to read {} metadata: {error}", target.display()),
                );
            }
        };

        let temp = match UpdateFile::temporary_path(&target, &update_id) {
            Ok(temp) => temp,

            Err(error) => {
                return self.fail_reserved_update(socket, executable, update, error);
            }
        };

        if temp.exists() {
            if let Err(error) = fs::remove_file(&temp) {
                return self.fail_reserved_update(
                    socket,
                    executable,
                    update,
                    format!("failed to remove stale {}: {error}", temp.display()),
                );
            }
        }

        let mut file = match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(file) => file,

            Err(error) => {
                return self.fail_reserved_update(
                    socket,
                    executable,
                    update,
                    format!("failed to create {}: {error}", temp.display()),
                );
            }
        };

        if let Err(error) = Self::send_response(
            socket,
            &ExecutableManagementResponse::Accepted {
                update_id: update_id.clone(),
                executable: executable.name_owned(),
            },
        ) {
            let _ = fs::remove_file(&temp);

            update.fail(error.clone());

            let _ = self.store.set_update(executable.name().as_str(), update);

            return Err(error);
        }

        let mut hasher = Sha256::new();

        while !update.is_fully_received() {
            let message = match socket.read() {
                Ok(message) => message,

                Err(error) => {
                    return self.fail_transfer(
                        socket,
                        executable,
                        update,
                        &temp,
                        format!("update transfer interrupted: {error}"),
                    );
                }
            };

            match message {
                WsMessage::Binary(payload) => {
                    let payload_size = payload.len() as u64;

                    if let Err(error) = update.receive(payload_size) {
                        return self.fail_transfer(socket, executable, update, &temp, error);
                    }

                    if let Err(error) = file.write_all(&payload) {
                        return self.fail_transfer(
                            socket,
                            executable,
                            update,
                            &temp,
                            format!("failed to write {}: {error}", temp.display()),
                        );
                    }

                    hasher.update(&payload);
                }

                WsMessage::Ping(payload) => {
                    if let Err(error) = socket.send_pong(&payload) {
                        return self.fail_transfer(
                            socket,
                            executable,
                            update,
                            &temp,
                            format!("failed to send Pong: {error}"),
                        );
                    }
                }

                WsMessage::Close => {
                    let received_size = update.received_size();

                    return self.fail_transfer(
                        socket,
                        executable,
                        update,
                        &temp,
                        format!(
                            "connection closed after {} of {} bytes",
                            received_size, expected_size
                        ),
                    );
                }

                _ => {
                    return self.fail_transfer(
                        socket,
                        executable,
                        update,
                        &temp,
                        "only binary frames are accepted during update transfer".to_string(),
                    );
                }
            }
        }

        if let Err(error) = file.flush() {
            return self.fail_transfer(
                socket,
                executable,
                update,
                &temp,
                format!("failed to flush {}: {error}", temp.display()),
            );
        }

        if let Err(error) = file.sync_all() {
            return self.fail_transfer(
                socket,
                executable,
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
            return self.fail_transfer(
                socket,
                executable,
                update,
                &temp,
                format!(
                    "SHA-256 mismatch: expected {}, got {}",
                    sha256, actual_sha256
                ),
            );
        }

        if let Err(error) = fs::set_permissions(&temp, permissions) {
            return self.fail_transfer(
                socket,
                executable,
                update,
                &temp,
                format!(
                    "failed to preserve executable permissions on {}: {error}",
                    temp.display()
                ),
            );
        }

        if let Err(error) = ExecutableBackup::of(&target).and_then(|backup| backup.prepare()) {
            return self.fail_transfer(socket, executable, update, &temp, error);
        }

        if let Err(error) = fs::rename(&temp, &target) {
            return self.fail_transfer(
                socket,
                executable,
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
        if let Err(error) = UpdateFile::sync_directory(parent) {
            update.keep_pending_with_error(error.clone());

            let _ = self.store.set_update(executable.name().as_str(), update);

            Self::send_failure(socket, &error)?;

            return Ok(());
        }

        update.commit();

        if let Err(error) = self.store.set_update(executable.name().as_str(), update) {
            Self::send_failure(
                socket,
                &format!("binary committed but committed state could not be persisted: {error}"),
            )?;

            return Ok(());
        }

        Self::send_response(
            socket,
            &ExecutableManagementResponse::Success {
                update_id,
                executable: executable.name_owned(),
            },
        )?;

        let _ = socket.close();

        Ok(())
    }

    fn fail_reserved_update(
        &self,
        socket: &mut WsConnection,
        executable: &Executable,
        mut update: Update,
        error: String,
    ) -> Result<(), String> {
        update.fail(error.clone());

        let _ = self.store.set_update(executable.name().as_str(), update);

        let _ = Self::send_failure(socket, &error);

        Ok(())
    }

    fn fail_transfer(
        &self,
        socket: &mut WsConnection,
        executable: &Executable,
        mut update: Update,
        temp: &Path,
        error: String,
    ) -> Result<(), String> {
        let _ = fs::remove_file(temp);

        update.fail(error.clone());

        let _ = self.store.set_update(executable.name().as_str(), update);

        let _ = Self::send_failure(socket, &error);

        Ok(())
    }

    fn send_failure(socket: &mut WsConnection, message: &str) -> Result<(), String> {
        Self::send_response(
            socket,
            &ExecutableManagementResponse::Failure {
                message: message.to_string(),
            },
        )
    }

    fn send_response(
        socket: &mut WsConnection,
        response: &ExecutableManagementResponse,
    ) -> Result<(), String> {
        let serialized = ron::ser::to_string(response)
            .map_err(|error| format!("failed to encode executable management response: {error}"))?;

        socket
            .send_text(&serialized)
            .map_err(|error| format!("failed to send executable management response: {error}"))
    }

    fn validate_update_id(value: &str) -> Result<(), String> {
        let uuid =
            Uuid::parse_str(value).map_err(|error| format!("invalid update UUID: {error}"))?;

        if uuid.get_version() != Some(Version::SortRand) {
            return Err(format!("update_id must be UUIDv7: {uuid}"));
        }

        Ok(())
    }

    fn validate_instance_id(value: &str) -> Result<(), String> {
        let uuid =
            Uuid::parse_str(value).map_err(|error| format!("invalid instance UUID: {error}"))?;

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
}

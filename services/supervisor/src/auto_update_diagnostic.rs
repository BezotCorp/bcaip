use crate::auto_update_phase::AutoUpdatePhase;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const LOG_FILE_NAME: &str = "supervisor-auto-update.log";

/// One event of a supervisor auto-update. Events are appended, one RON
/// document per line, so every attempt and every rollback stays available.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct AutoUpdateDiagnostic {
    timestamp_unix_ms: u128,
    update_id: String,
    executable: String,
    phase: AutoUpdatePhase,
    cause: Option<AutoUpdatePhase>,
    candidate_pid: Option<u32>,
    candidate_instance_id: Option<String>,
    error: Option<String>,
    runtime_backup: Option<String>,
    rollback_error: Option<String>,
}

impl AutoUpdateDiagnostic {
    pub(crate) fn new(update_id: &str, executable: &str, phase: AutoUpdatePhase) -> Self {
        Self {
            timestamp_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            update_id: update_id.to_string(),
            executable: executable.to_string(),
            phase,
            cause: None,
            candidate_pid: None,
            candidate_instance_id: None,
            error: None,
            runtime_backup: None,
            rollback_error: None,
        }
    }

    pub(crate) fn with_error(mut self, error: String) -> Self {
        self.error = Some(error);
        self
    }

    pub(crate) fn with_cause(mut self, cause: AutoUpdatePhase) -> Self {
        self.cause = Some(cause);
        self
    }

    pub(crate) fn with_candidate(mut self, pid: Option<u32>, instance_id: &str) -> Self {
        self.candidate_pid = pid;
        self.candidate_instance_id = Some(instance_id.to_string());
        self
    }

    pub(crate) fn with_runtime_backup(mut self, path: &Path) -> Self {
        self.runtime_backup = Some(path.display().to_string());
        self
    }

    pub(crate) fn with_rollback_error(mut self, error: String) -> Self {
        self.rollback_error = Some(error);
        self
    }

    pub(crate) fn append(&self, data_directory: &Path) -> Result<(), String> {
        fs::create_dir_all(data_directory).map_err(|error| {
            format!(
                "failed to create diagnostic directory {}: {error}",
                data_directory.display()
            )
        })?;

        let target = data_directory.join(LOG_FILE_NAME);
        let mut line = ron::ser::to_string(self)
            .map_err(|error| format!("failed to encode auto-update diagnostic: {error}"))?;
        line.push('\n');

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&target)
            .map_err(|error| format!("failed to open {}: {error}", target.display()))?;

        file.write_all(line.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("failed to write {}: {error}", target.display()))
    }
}

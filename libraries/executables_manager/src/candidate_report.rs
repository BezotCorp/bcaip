use serde::{Deserialize, Serialize};

/// What a candidate states about itself; the authoritative supervisor
/// compares it with its own view before promoting.
#[derive(Debug, Deserialize, Serialize)]
pub struct CandidateReport {
    executable: String,
    instance_id: String,
    applied_update_id: Option<String>,
    writer: bool,
    runtime_fingerprint: String,
}

impl CandidateReport {
    pub fn new(
        executable: String,
        instance_id: String,
        applied_update_id: Option<String>,
        writer: bool,
        runtime_fingerprint: String,
    ) -> Self {
        Self {
            executable,
            instance_id,
            applied_update_id,
            writer,
            runtime_fingerprint,
        }
    }

    pub fn check(
        &self,
        executable: &str,
        instance_id: &str,
        update_id: &str,
        runtime_fingerprint: &str,
    ) -> Result<(), String> {
        if self.executable != executable || self.instance_id != instance_id {
            return Err(format!(
                "candidate identity mismatch: {} / {}",
                self.executable, self.instance_id
            ));
        }

        if self.applied_update_id.as_deref() != Some(update_id) {
            return Err(format!(
                "candidate runs update {:?}, expected {update_id}",
                self.applied_update_id
            ));
        }

        if self.writer {
            return Err("candidate claims write authority before promotion".to_string());
        }

        if self.runtime_fingerprint != runtime_fingerprint {
            return Err(
                "candidate did not load the same runtime state as the authoritative supervisor"
                    .to_string(),
            );
        }

        Ok(())
    }
}

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) enum SupervisorHealthResponse {
    Healthy {
        executable: String,
        instance_id: String,
        applied_update_id: Option<String>,
    },

    Failure {
        message: String,
    },
}

impl SupervisorHealthResponse {
    pub(crate) fn is_healthy_supervisor(&self) -> bool {
        match self {
            Self::Healthy {
                executable,
                instance_id,
                applied_update_id,
            } => {
                let _ = applied_update_id;

                executable == "supervisor" && !instance_id.is_empty()
            }

            Self::Failure { message } => {
                let _ = message;
                false
            }
        }
    }
}

use crate::executable::Executable;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub(crate) enum WatcherResponse {
    Healthy {
        executable: String,
        instance_id: String,
        applied_update_id: Option<String>,
    },

    Accepted {
        update_id: String,
        executable: String,
    },

    Success {
        update_id: String,
        executable: String,
    },

    Status {
        executable: Executable,
    },

    Acknowledged,

    Failure {
        message: String,
    },
}

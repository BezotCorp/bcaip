use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Deserialize, Serialize, Debug)]
pub(crate) enum ExecutableManagementRequest {
    Health,

    CandidateReport,

    PromoteCandidate {
        executable: String,
        instance_id: String,
    },

    ExpectProcess {
        executable: String,
        instance_id: String,
    },

    BeginUpdate {
        update_id: String,
        executable: String,
        expected_size: u64,
        sha256: String,
    },

    Status {
        executable: String,
    },

    ProcessStarted {
        executable: String,
        pid: u32,
        instance_id: String,
        ws_address: SocketAddr,
        applied_update_id: Option<String>,
    },

    ProcessStopped {
        executable: String,
        instance_id: String,
    },
}

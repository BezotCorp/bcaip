use crate::candidate_report::CandidateReport;
use crate::executable::Executable;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub(crate) enum ExecutableManagementResponse {
    Healthy {
        executable: String,
        instance_id: String,
        applied_update_id: Option<String>,
    },

    Candidate(CandidateReport),

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

    Promoted,

    Failure {
        message: String,
    },
}

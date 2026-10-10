use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub(crate) enum UpdatePhase {
    Pending,
    Committed,
    Failed,
}

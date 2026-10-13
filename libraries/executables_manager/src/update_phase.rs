use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Encode, Decode, Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub enum UpdatePhase {
    Pending,
    Committed,
    Failed,
}

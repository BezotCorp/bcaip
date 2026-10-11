use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Decode, Deserialize, Encode, Eq, PartialEq, Serialize)]
pub(crate) enum UpdatePhase {
    Pending,
    Committed,
    Failed,
}

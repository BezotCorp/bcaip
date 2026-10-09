use bincode_next::{Decode, Encode};

use crate::StorageOutcome;

#[derive(Debug, Clone, Encode, Decode)]
pub struct StorageResponse {
    pub id: u64,
    pub outcome: StorageOutcome,
}

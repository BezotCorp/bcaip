use bincode_next::{Decode, Encode};

use crate::StorageOperation;

#[derive(Debug, Clone, Encode, Decode)]
pub struct StorageRequest {
    pub id: u64,
    pub operation: StorageOperation,
}

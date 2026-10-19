use bincode_next::{Decode, Encode};

use crate::ExchangeError;

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub enum StorageOutcome {
    Pong,
    Created { revision: i64 },
    Data { content: Vec<u8>, revision: i64 },
    Updated { revision: i64 },
    Deleted,
    Error(ExchangeError),
}

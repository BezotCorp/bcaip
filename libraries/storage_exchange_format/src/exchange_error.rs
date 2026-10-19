use bincode_next::{Decode, Encode};

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub enum ExchangeError {
    InvalidRequest,
    AlreadyExists,
    NotFound,
    RevisionConflict,
    StorageFailure,
}

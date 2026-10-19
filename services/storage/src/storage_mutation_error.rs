use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum StorageMutationError {
    AlreadyExists,
    NotFound,
    RevisionConflict,
}

impl fmt::Display for StorageMutationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyExists => write!(f, "record already exists"),
            Self::NotFound => write!(f, "record not found"),
            Self::RevisionConflict => write!(f, "revision conflict"),
        }
    }
}

impl Error for StorageMutationError {}

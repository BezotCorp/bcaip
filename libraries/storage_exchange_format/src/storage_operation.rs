use bincode_next::{Decode, Encode};

#[derive(Debug, Clone, Encode, Decode)]
pub enum StorageOperation {
    Ping,
    Create {
        namespace: String,
        key: String,
        content: Vec<u8>,
    },
    Read {
        namespace: String,
        key: String,
    },
    Update {
        namespace: String,
        key: String,
        content: Vec<u8>,
        expected_revision: i64,
    },
    Delete {
        namespace: String,
        key: String,
        expected_revision: i64,
    },
}

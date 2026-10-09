#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataRecord {
    pub namespace: String,
    pub key: String,
    pub content: Vec<u8>,
    pub revision: i64,
}

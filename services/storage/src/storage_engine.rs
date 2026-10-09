use std::error::Error;

use crate::data_record::DataRecord;

pub type StorageResult<T> = Result<T, Box<dyn Error>>;

pub trait StorageEngine {
    fn read(&self, namespace: &str, key: &str) -> StorageResult<Option<DataRecord>>;

    fn write(
        &mut self,
        namespace: &str,
        key: &str,
        content: &[u8],
        expected_revision: Option<i64>,
    ) -> StorageResult<DataRecord>;

    fn delete(&mut self, namespace: &str, key: &str, expected_revision: i64)
    -> StorageResult<bool>;
}

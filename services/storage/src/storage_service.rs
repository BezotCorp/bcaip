use std::collections::{HashMap, VecDeque};

use crate::data_record::DataRecord;
use crate::storage_engine::{StorageEngine, StorageResult};

const DEFAULT_CACHE_CAPACITY: usize = 1_024;

pub struct StorageService<E: StorageEngine> {
    engine: E,
    records: HashMap<(String, String), Option<DataRecord>>,
    cache_order: VecDeque<(String, String)>,
    cache_capacity: usize,
}

impl<E: StorageEngine> StorageService<E> {
    pub fn new(engine: E) -> Self {
        Self::with_cache_capacity(engine, DEFAULT_CACHE_CAPACITY)
    }

    pub fn with_cache_capacity(engine: E, cache_capacity: usize) -> Self {
        Self {
            engine,
            records: HashMap::new(),
            cache_order: VecDeque::new(),
            cache_capacity,
        }
    }

    pub fn read(&mut self, namespace: &str, key: &str) -> StorageResult<Option<DataRecord>> {
        let cache_key = (namespace.to_owned(), key.to_owned());

        if let Some(record) = self.records.get(&cache_key).cloned() {
            self.touch(&cache_key);
            return Ok(record);
        }

        let record = self.engine.read(namespace, key)?;

        self.cache(cache_key, record.clone());

        Ok(record)
    }

    pub fn create(
        &mut self,
        namespace: &str,
        key: &str,
        content: &[u8],
    ) -> StorageResult<DataRecord> {
        let record = self.engine.write(namespace, key, content, None)?;

        self.cache(
            (namespace.to_owned(), key.to_owned()),
            Some(record.clone()),
        );

        Ok(record)
    }

    pub fn update(
        &mut self,
        namespace: &str,
        key: &str,
        content: &[u8],
        revision: i64,
    ) -> StorageResult<DataRecord> {
        let record = self
            .engine
            .write(namespace, key, content, Some(revision))?;

        self.cache(
            (namespace.to_owned(), key.to_owned()),
            Some(record.clone()),
        );

        Ok(record)
    }

    pub fn delete(
        &mut self,
        namespace: &str,
        key: &str,
        revision: i64,
    ) -> StorageResult<bool> {
        let deleted = self.engine.delete(namespace, key, revision)?;

        if deleted {
            self.cache(
                (namespace.to_owned(), key.to_owned()),
                None,
            );
        }

        Ok(deleted)
    }

    fn cache(
        &mut self,
        key: (String, String),
        record: Option<DataRecord>,
    ) {
        if self.cache_capacity == 0 {
            return;
        }

        self.records.insert(key.clone(), record);
        self.touch(&key);

        while self.records.len() > self.cache_capacity {
            let Some(oldest) = self.cache_order.pop_front() else {
                break;
            };

            self.records.remove(&oldest);
        }
    }

    fn touch(&mut self, key: &(String, String)) {
        if let Some(position) = self
            .cache_order
            .iter()
            .position(|existing| existing == key)
        {
            self.cache_order.remove(position);
        }

        self.cache_order.push_back(key.clone());
    }
}

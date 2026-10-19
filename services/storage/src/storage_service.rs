use std::collections::{HashMap, VecDeque};
use std::mem::size_of;

use crate::data_record::DataRecord;
use crate::storage_engine::{StorageEngine, StorageResult};

type CacheKey = (String, String);

pub struct StorageService<E: StorageEngine> {
    engine: E,
    records: HashMap<CacheKey, Option<DataRecord>>,
    cache_order: VecDeque<CacheKey>,
    cache_capacity: usize,
    cache_max_bytes: usize,
    cache_bytes: usize,
}

impl<E: StorageEngine> StorageService<E> {
    pub fn with_cache_capacity(engine: E, cache_capacity: usize) -> Self {
        Self::with_cache_limits(engine, cache_capacity, usize::MAX)
    }

    pub fn with_cache_limits(engine: E, cache_capacity: usize, cache_max_bytes: usize) -> Self {
        Self {
            engine,
            records: HashMap::new(),
            cache_order: VecDeque::new(),
            cache_capacity,
            cache_max_bytes,
            cache_bytes: 0,
        }
    }

    pub fn cached_state(&mut self, namespace: &str, key: &str) -> Option<Option<DataRecord>> {
        let cache_key = (namespace.to_owned(), key.to_owned());
        let record = self.records.get(&cache_key).cloned()?;

        self.touch(&cache_key);

        Some(record)
    }

    pub fn read(&mut self, namespace: &str, key: &str) -> StorageResult<Option<DataRecord>> {
        if let Some(record) = self.cached_state(namespace, key) {
            return Ok(record);
        }

        self.refresh(namespace, key)
    }

    pub fn refresh(&mut self, namespace: &str, key: &str) -> StorageResult<Option<DataRecord>> {
        let cache_key = (namespace.to_owned(), key.to_owned());
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

        self.cache((namespace.to_owned(), key.to_owned()), Some(record.clone()));

        Ok(record)
    }

    pub fn update(
        &mut self,
        namespace: &str,
        key: &str,
        content: &[u8],
        revision: i64,
    ) -> StorageResult<DataRecord> {
        let record = self.engine.write(namespace, key, content, Some(revision))?;

        self.cache((namespace.to_owned(), key.to_owned()), Some(record.clone()));

        Ok(record)
    }

    pub fn delete(&mut self, namespace: &str, key: &str, revision: i64) -> StorageResult<bool> {
        let deleted = self.engine.delete(namespace, key, revision)?;

        if deleted {
            self.cache((namespace.to_owned(), key.to_owned()), None);
        }

        Ok(deleted)
    }

    fn cache(&mut self, key: CacheKey, record: Option<DataRecord>) {
        self.remove_cached(&key);

        if self.cache_capacity == 0 || self.cache_max_bytes == 0 {
            return;
        }

        let weight = Self::cache_weight(&key, &record);

        if weight > self.cache_max_bytes {
            return;
        }

        self.cache_bytes += weight;
        self.records.insert(key.clone(), record);
        self.cache_order.push_back(key);

        self.evict_to_limits();
    }

    fn touch(&mut self, key: &CacheKey) {
        if let Some(position) = self.cache_order.iter().position(|existing| existing == key) {
            self.cache_order.remove(position);
            self.cache_order.push_back(key.clone());
        }
    }

    fn remove_cached(&mut self, key: &CacheKey) {
        if let Some(record) = self.records.remove(key) {
            self.cache_bytes = self
                .cache_bytes
                .saturating_sub(Self::cache_weight(key, &record));
        }

        if let Some(position) = self.cache_order.iter().position(|existing| existing == key) {
            self.cache_order.remove(position);
        }
    }

    fn evict_to_limits(&mut self) {
        while self.records.len() > self.cache_capacity || self.cache_bytes > self.cache_max_bytes {
            let Some(oldest) = self.cache_order.pop_front() else {
                break;
            };

            if let Some(record) = self.records.remove(&oldest) {
                self.cache_bytes = self
                    .cache_bytes
                    .saturating_sub(Self::cache_weight(&oldest, &record));
            }
        }
    }

    fn cache_weight(key: &CacheKey, record: &Option<DataRecord>) -> usize {
        let key_weight = key.0.len().saturating_add(key.1.len());

        match record {
            Some(record) => key_weight
                .saturating_add(record.namespace.len())
                .saturating_add(record.key.len())
                .saturating_add(record.content.len())
                .saturating_add(size_of::<i64>()),
            None => key_weight,
        }
    }
}

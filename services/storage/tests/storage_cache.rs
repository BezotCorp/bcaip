use std::collections::HashMap;
use std::error::Error;

#[path = "../src/data_record.rs"]
mod data_record;

#[path = "../src/storage_engine.rs"]
mod storage_engine;

#[path = "../src/storage_service.rs"]
mod storage_service;

use data_record::DataRecord;
use storage_engine::{StorageEngine, StorageResult};
use storage_service::StorageService;

#[derive(Default)]
struct CountingEngine {
    records: HashMap<(String, String), DataRecord>,
    reads: usize,
    writes: usize,
    deletes: usize,
}

impl StorageEngine for CountingEngine {
    fn read(
        &self,
        _namespace: &str,
        _key: &str,
    ) -> StorageResult<Option<DataRecord>> {
        unreachable!("read nécessite un moteur mutable pour compter")
    }

    fn write(
        &mut self,
        namespace: &str,
        key: &str,
        content: &[u8],
        expected_revision: Option<i64>,
    ) -> StorageResult<DataRecord> {
        self.writes += 1;

        let map_key = (namespace.to_owned(), key.to_owned());

        let revision = match expected_revision {
            None => {
                if self.records.contains_key(&map_key) {
                    return Err("Already exists".into());
                }

                1
            }
            Some(expected) => {
                let current = self
                    .records
                    .get(&map_key)
                    .ok_or("Not found")?;

                if current.revision != expected {
                    return Err("Revision conflict".into());
                }

                expected.checked_add(1).ok_or("Revision overflow")?
            }
        };

        let record = DataRecord {
            namespace: namespace.to_owned(),
            key: key.to_owned(),
            content: content.to_vec(),
            revision,
        };

        self.records.insert(map_key, record.clone());

        Ok(record)
    }

    fn delete(
        &mut self,
        namespace: &str,
        key: &str,
        expected_revision: i64,
    ) -> StorageResult<bool> {
        self.deletes += 1;

        let map_key = (namespace.to_owned(), key.to_owned());

        let Some(current) = self.records.get(&map_key) else {
            return Ok(false);
        };

        if current.revision != expected_revision {
            return Ok(false);
        }

        self.records.remove(&map_key);

        Ok(true)
    }
}

struct SharedCountingEngine {
    state: std::rc::Rc<std::cell::RefCell<CountingEngine>>,
}

impl SharedCountingEngine {
    fn new(
        state: std::rc::Rc<std::cell::RefCell<CountingEngine>>,
    ) -> Self {
        Self { state }
    }
}

impl StorageEngine for SharedCountingEngine {
    fn read(
        &self,
        namespace: &str,
        key: &str,
    ) -> StorageResult<Option<DataRecord>> {
        let mut state = self.state.borrow_mut();
        state.reads += 1;

        Ok(state
            .records
            .get(&(namespace.to_owned(), key.to_owned()))
            .cloned())
    }

    fn write(
        &mut self,
        namespace: &str,
        key: &str,
        content: &[u8],
        expected_revision: Option<i64>,
    ) -> StorageResult<DataRecord> {
        self.state.borrow_mut().write(
            namespace,
            key,
            content,
            expected_revision,
        )
    }

    fn delete(
        &mut self,
        namespace: &str,
        key: &str,
        expected_revision: i64,
    ) -> StorageResult<bool> {
        self.state
            .borrow_mut()
            .delete(namespace, key, expected_revision)
    }
}

fn service(
    capacity: usize,
) -> (
    StorageService<SharedCountingEngine>,
    std::rc::Rc<std::cell::RefCell<CountingEngine>>,
) {
    let state =
        std::rc::Rc::new(std::cell::RefCell::new(CountingEngine::default()));

    let engine = SharedCountingEngine::new(state.clone());

    (
        StorageService::with_cache_capacity(engine, capacity),
        state,
    )
}

#[test]
fn repeated_reads_use_one_engine_read() {
    let (mut storage, engine) = service(16);

    engine.borrow_mut().records.insert(
        ("records".into(), "item".into()),
        DataRecord {
            namespace: "records".into(),
            key: "item".into(),
            content: vec![1],
            revision: 1,
        },
    );

    for _ in 0..10 {
        assert_eq!(
            storage.read("records", "item").unwrap(),
            Some(DataRecord {
                namespace: "records".into(),
                key: "item".into(),
                content: vec![1],
                revision: 1,
            }),
        );
    }

    assert_eq!(engine.borrow().reads, 1);
}

#[test]
fn repeated_missing_reads_use_one_engine_read() {
    let (mut storage, engine) = service(16);

    for _ in 0..10 {
        assert_eq!(
            storage.read("records", "missing").unwrap(),
            None,
        );
    }

    assert_eq!(engine.borrow().reads, 1);
}

#[test]
fn successful_write_refreshes_cached_record() {
    let (mut storage, engine) = service(16);

    let created = storage
        .create("records", "item", &[1])
        .unwrap();

    assert_eq!(created.revision, 1);
    assert_eq!(engine.borrow().writes, 1);

    let record = storage
        .read("records", "item")
        .unwrap()
        .unwrap();

    assert_eq!(record.content, vec![1]);
    assert_eq!(record.revision, 1);

    assert_eq!(
        engine.borrow().reads,
        0,
        "Une création réussie doit alimenter le cache",
    );

    let updated = storage
        .update("records", "item", &[2], 1)
        .unwrap();

    assert_eq!(updated.revision, 2);
    assert_eq!(engine.borrow().writes, 2);

    let record = storage
        .read("records", "item")
        .unwrap()
        .unwrap();

    assert_eq!(record.content, vec![2]);
    assert_eq!(record.revision, 2);

    assert_eq!(
        engine.borrow().reads,
        0,
        "Une mise à jour réussie doit remplacer l'entrée du cache",
    );
}

#[test]
fn successful_delete_caches_known_absence() {
    let (mut storage, engine) = service(16);

    storage
        .create("records", "item", &[1])
        .unwrap();

    assert!(
        storage
            .delete("records", "item", 1)
            .unwrap()
    );

    assert_eq!(engine.borrow().deletes, 1);

    for _ in 0..10 {
        assert_eq!(
            storage.read("records", "item").unwrap(),
            None,
        );
    }

    assert_eq!(
        engine.borrow().reads,
        0,
        "Une suppression réussie rend l'absence connue",
    );
}

#[test]
fn cache_capacity_evicts_old_entries() {
    let (mut storage, engine) = service(2);

    for key in ["first", "second", "third"] {
        engine.borrow_mut().records.insert(
            ("records".into(), key.into()),
            DataRecord {
                namespace: "records".into(),
                key: key.into(),
                content: key.as_bytes().to_vec(),
                revision: 1,
            },
        );

        assert!(
            storage
                .read("records", key)
                .unwrap()
                .is_some()
        );
    }

    assert_eq!(
        engine.borrow().reads,
        3,
        "Les trois premières lectures sont des cache misses",
    );

    assert!(
        storage
            .read("records", "first")
            .unwrap()
            .is_some()
    );

    assert_eq!(
        engine.borrow().reads,
        4,
        "Avec une capacité de 2, une des anciennes entrées doit avoir été évincée",
    );
}

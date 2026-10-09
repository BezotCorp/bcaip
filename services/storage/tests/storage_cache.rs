use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

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
    read_failures_remaining: usize,
}

impl CountingEngine {
    fn insert(&mut self, namespace: &str, key: &str, content: Vec<u8>, revision: i64) {
        self.records.insert(
            (namespace.to_owned(), key.to_owned()),
            DataRecord {
                namespace: namespace.to_owned(),
                key: key.to_owned(),
                content,
                revision,
            },
        );
    }
}

struct SharedCountingEngine {
    state: Rc<RefCell<CountingEngine>>,
}

impl SharedCountingEngine {
    fn new(state: Rc<RefCell<CountingEngine>>) -> Self {
        Self { state }
    }
}

impl StorageEngine for SharedCountingEngine {
    fn read(&self, namespace: &str, key: &str) -> StorageResult<Option<DataRecord>> {
        let mut state = self.state.borrow_mut();

        state.reads += 1;

        if state.read_failures_remaining > 0 {
            state.read_failures_remaining -= 1;
            return Err("Injected read failure".into());
        }

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
        let mut state = self.state.borrow_mut();

        state.writes += 1;

        let map_key = (namespace.to_owned(), key.to_owned());

        let revision = match expected_revision {
            None => {
                if state.records.contains_key(&map_key) {
                    return Err("Already exists".into());
                }

                1
            }
            Some(expected_revision) => {
                let current = state.records.get(&map_key).ok_or("Not found")?;

                if current.revision != expected_revision {
                    return Err("Revision conflict".into());
                }

                expected_revision
                    .checked_add(1)
                    .ok_or("Revision overflow")?
            }
        };

        let record = DataRecord {
            namespace: namespace.to_owned(),
            key: key.to_owned(),
            content: content.to_vec(),
            revision,
        };

        state.records.insert(map_key, record.clone());

        Ok(record)
    }

    fn delete(
        &mut self,
        namespace: &str,
        key: &str,
        expected_revision: i64,
    ) -> StorageResult<bool> {
        let mut state = self.state.borrow_mut();

        state.deletes += 1;

        let map_key = (namespace.to_owned(), key.to_owned());

        let Some(current) = state.records.get(&map_key) else {
            return Ok(false);
        };

        if current.revision != expected_revision {
            return Ok(false);
        }

        state.records.remove(&map_key);

        Ok(true)
    }
}

fn service(
    capacity: usize,
) -> (
    StorageService<SharedCountingEngine>,
    Rc<RefCell<CountingEngine>>,
) {
    service_with_limits(capacity, usize::MAX)
}

fn service_with_limits(
    capacity: usize,
    max_bytes: usize,
) -> (
    StorageService<SharedCountingEngine>,
    Rc<RefCell<CountingEngine>>,
) {
    let state = Rc::new(RefCell::new(CountingEngine::default()));

    let engine = SharedCountingEngine::new(state.clone());

    (
        StorageService::with_cache_limits(engine, capacity, max_bytes),
        state,
    )
}

#[test]
fn repeated_reads_use_one_engine_read() {
    let (mut storage, engine) = service(16);

    engine.borrow_mut().insert("records", "item", vec![1], 1);

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
        assert_eq!(storage.read("records", "missing").unwrap(), None,);
    }

    assert_eq!(engine.borrow().reads, 1);
}

#[test]
fn successful_write_refreshes_cached_record() {
    let (mut storage, engine) = service(16);

    let created = storage.create("records", "item", &[1]).unwrap();

    assert_eq!(created.revision, 1);
    assert_eq!(engine.borrow().writes, 1);

    let record = storage.read("records", "item").unwrap().unwrap();

    assert_eq!(record.content, vec![1]);
    assert_eq!(record.revision, 1);
    assert_eq!(engine.borrow().reads, 0);

    let updated = storage.update("records", "item", &[2], 1).unwrap();

    assert_eq!(updated.revision, 2);
    assert_eq!(engine.borrow().writes, 2);

    let record = storage.read("records", "item").unwrap().unwrap();

    assert_eq!(record.content, vec![2]);
    assert_eq!(record.revision, 2);
    assert_eq!(engine.borrow().reads, 0);
}

#[test]
fn successful_delete_caches_known_absence() {
    let (mut storage, engine) = service(16);

    storage.create("records", "item", &[1]).unwrap();

    assert!(storage.delete("records", "item", 1).unwrap());

    assert_eq!(engine.borrow().deletes, 1);

    for _ in 0..10 {
        assert_eq!(storage.read("records", "item").unwrap(), None,);
    }

    assert_eq!(engine.borrow().reads, 0);
}

#[test]
fn cache_capacity_evicts_old_entries() {
    let (mut storage, engine) = service(2);

    for key in ["first", "second", "third"] {
        engine.borrow_mut().insert("records", key, vec![1], 1);

        assert!(storage.read("records", key).unwrap().is_some());
    }

    assert_eq!(engine.borrow().reads, 3);

    assert!(storage.read("records", "first").unwrap().is_some());

    assert_eq!(engine.borrow().reads, 4);
}

#[test]
fn cache_eviction_uses_least_recently_used_order() {
    let (mut storage, engine) = service(2);

    for key in ["first", "second", "third"] {
        engine.borrow_mut().insert("records", key, vec![1], 1);
    }

    storage.read("records", "first").unwrap();
    storage.read("records", "second").unwrap();

    assert_eq!(engine.borrow().reads, 2);

    storage.read("records", "first").unwrap();

    assert_eq!(
        engine.borrow().reads,
        2,
        "Relire first doit seulement rafraîchir son rang LRU",
    );

    storage.read("records", "third").unwrap();

    assert_eq!(engine.borrow().reads, 3);

    storage.read("records", "first").unwrap();

    assert_eq!(
        engine.borrow().reads,
        3,
        "first était récemment utilisé et doit encore être caché",
    );

    storage.read("records", "second").unwrap();

    assert_eq!(
        engine.borrow().reads,
        4,
        "second était la moins récemment utilisée et doit avoir été évincée",
    );
}

#[test]
fn zero_entry_capacity_disables_cache() {
    let (mut storage, engine) = service(0);

    engine.borrow_mut().insert("records", "item", vec![1], 1);

    for _ in 0..10 {
        assert!(storage.read("records", "item").unwrap().is_some());
    }

    assert_eq!(engine.borrow().reads, 10);
}

#[test]
fn zero_byte_budget_disables_cache() {
    let (mut storage, engine) = service_with_limits(16, 0);

    engine.borrow_mut().insert("records", "item", vec![1], 1);

    for _ in 0..10 {
        assert!(storage.read("records", "item").unwrap().is_some());
    }

    assert_eq!(engine.borrow().reads, 10);
}

#[test]
fn read_failure_is_never_cached_as_absence() {
    let (mut storage, engine) = service(16);

    engine.borrow_mut().insert("records", "item", vec![1], 1);

    engine.borrow_mut().read_failures_remaining = 1;

    assert!(storage.read("records", "item").is_err());

    assert_eq!(engine.borrow().reads, 1);

    let record = storage.read("records", "item").unwrap().unwrap();

    assert_eq!(record.content, vec![1]);

    assert_eq!(
        engine.borrow().reads,
        2,
        "Après une erreur moteur, Storage doit réellement réessayer",
    );

    storage.read("records", "item").unwrap();

    assert_eq!(
        engine.borrow().reads,
        2,
        "Le résultat réussi peut ensuite être caché",
    );
}

#[test]
fn failed_update_keeps_previous_cached_record() {
    let (mut storage, engine) = service(16);

    storage.create("records", "item", &[1]).unwrap();

    let result = storage.update("records", "item", &[9], 99);

    assert!(result.is_err());

    let record = storage.read("records", "item").unwrap().unwrap();

    assert_eq!(record.content, vec![1]);
    assert_eq!(record.revision, 1);

    assert_eq!(
        engine.borrow().reads,
        0,
        "Une écriture échouée ne doit pas invalider un cache encore correct",
    );
}

#[test]
fn oversized_record_is_not_cached() {
    let content = vec![7; 256];

    let namespace = "records";
    let key = "large";

    let logical_weight = namespace.len()
        + key.len()
        + namespace.len()
        + key.len()
        + content.len()
        + std::mem::size_of::<i64>();

    let (mut storage, engine) = service_with_limits(16, logical_weight - 1);

    engine.borrow_mut().insert(namespace, key, content, 1);

    storage.read(namespace, key).unwrap();
    storage.read(namespace, key).unwrap();

    assert_eq!(
        engine.borrow().reads,
        2,
        "Une entrée plus grosse que le budget ne doit jamais rester en cache",
    );
}

#[test]
fn byte_budget_evicts_entries_before_entry_limit() {
    let namespace = "r";
    let first = "a";
    let second = "b";

    let one_record_weight = namespace.len()
        + first.len()
        + namespace.len()
        + first.len()
        + 32
        + std::mem::size_of::<i64>();

    let (mut storage, engine) = service_with_limits(100, one_record_weight);

    engine.borrow_mut().insert(namespace, first, vec![1; 32], 1);

    engine
        .borrow_mut()
        .insert(namespace, second, vec![2; 32], 1);

    storage.read(namespace, first).unwrap();
    storage.read(namespace, second).unwrap();

    assert_eq!(engine.borrow().reads, 2);

    storage.read(namespace, first).unwrap();

    assert_eq!(
        engine.borrow().reads,
        3,
        "Le budget mémoire doit évincer même si la limite d'entrées n'est pas atteinte",
    );
}

#[test]
fn missing_entries_are_also_subject_to_eviction() {
    let (mut storage, engine) = service(2);

    assert_eq!(storage.read("records", "missing-a").unwrap(), None,);

    assert_eq!(storage.read("records", "missing-b").unwrap(), None,);

    assert_eq!(storage.read("records", "missing-c").unwrap(), None,);

    assert_eq!(engine.borrow().reads, 3);

    assert_eq!(storage.read("records", "missing-a").unwrap(), None,);

    assert_eq!(
        engine.borrow().reads,
        4,
        "Les absences connues ne doivent pas rester en cache sans limite",
    );
}

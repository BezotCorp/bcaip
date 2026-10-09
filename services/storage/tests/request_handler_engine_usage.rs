use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[path = "../src/data_record.rs"]
mod data_record;

#[path = "../src/request_handler.rs"]
mod request_handler;

#[path = "../src/storage_engine.rs"]
mod storage_engine;

#[path = "../src/storage_service.rs"]
mod storage_service;

use data_record::DataRecord;
use request_handler::handle_request;
use storage_engine::{StorageEngine, StorageResult};
use storage_exchange_format::{
    ExchangeError, StorageOperation, StorageOutcome, StorageRequest,
};
use storage_service::StorageService;

#[derive(Default)]
struct CountingEngine {
    records: HashMap<(String, String), DataRecord>,
    reads: usize,
    writes: usize,
    deletes: usize,
    write_failures_remaining: usize,
    delete_failures_remaining: usize,
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

        if state.write_failures_remaining > 0 {
            state.write_failures_remaining -= 1;
            return Err("Injected write failure".into());
        }

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

        if state.delete_failures_remaining > 0 {
            state.delete_failures_remaining -= 1;
            return Err("Injected delete failure".into());
        }

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

fn service() -> (
    StorageService<SharedCountingEngine>,
    Rc<RefCell<CountingEngine>>,
) {
    let state = Rc::new(RefCell::new(CountingEngine::default()));
    let engine = SharedCountingEngine::new(state.clone());

    (
        StorageService::with_cache_limits(engine, 128, 1024 * 1024),
        state,
    )
}

fn request(id: u64, operation: StorageOperation) -> StorageRequest {
    StorageRequest { id, operation }
}

#[test]
fn successful_cold_create_uses_one_engine_write_and_no_read() {
    let (mut storage, engine) = service();

    let response = handle_request(
        &mut storage,
        request(
            1,
            StorageOperation::Create {
                namespace: "records".into(),
                key: "item".into(),
                content: vec![1],
            },
        ),
    );

    assert_eq!(response.outcome, StorageOutcome::Created { revision: 1 });

    let engine = engine.borrow();
    assert_eq!(engine.reads, 0);
    assert_eq!(engine.writes, 1);
    assert_eq!(engine.deletes, 0);
}

#[test]
fn cached_duplicate_create_does_not_touch_engine_again() {
    let (mut storage, engine) = service();

    assert_eq!(
        handle_request(
            &mut storage,
            request(
                1,
                StorageOperation::Create {
                    namespace: "records".into(),
                    key: "item".into(),
                    content: vec![1],
                },
            ),
        )
        .outcome,
        StorageOutcome::Created { revision: 1 },
    );

    assert_eq!(
        handle_request(
            &mut storage,
            request(
                2,
                StorageOperation::Create {
                    namespace: "records".into(),
                    key: "item".into(),
                    content: vec![9],
                },
            ),
        )
        .outcome,
        StorageOutcome::Error(ExchangeError::AlreadyExists),
    );

    let engine = engine.borrow();
    assert_eq!(engine.reads, 0);
    assert_eq!(engine.writes, 1);
}

#[test]
fn cold_duplicate_create_uses_one_write_then_one_read_to_classify_failure() {
    let (mut storage, engine) = service();

    engine.borrow_mut().insert("records", "item", vec![1], 1);

    assert_eq!(
        handle_request(
            &mut storage,
            request(
                1,
                StorageOperation::Create {
                    namespace: "records".into(),
                    key: "item".into(),
                    content: vec![9],
                },
            ),
        )
        .outcome,
        StorageOutcome::Error(ExchangeError::AlreadyExists),
    );

    let engine = engine.borrow();
    assert_eq!(engine.writes, 1);
    assert_eq!(engine.reads, 1);
}

#[test]
fn create_engine_failure_is_storage_failure() {
    let (mut storage, engine) = service();

    engine.borrow_mut().write_failures_remaining = 1;

    assert_eq!(
        handle_request(
            &mut storage,
            request(
                1,
                StorageOperation::Create {
                    namespace: "records".into(),
                    key: "item".into(),
                    content: vec![1],
                },
            ),
        )
        .outcome,
        StorageOutcome::Error(ExchangeError::StorageFailure),
    );

    let engine = engine.borrow();
    assert_eq!(engine.writes, 1);
    assert_eq!(engine.reads, 1);
}

#[test]
fn successful_cold_update_uses_one_engine_write_and_no_read() {
    let (mut storage, engine) = service();

    engine.borrow_mut().insert("records", "item", vec![1], 1);

    assert_eq!(
        handle_request(
            &mut storage,
            request(
                1,
                StorageOperation::Update {
                    namespace: "records".into(),
                    key: "item".into(),
                    content: vec![2],
                    expected_revision: 1,
                },
            ),
        )
        .outcome,
        StorageOutcome::Updated { revision: 2 },
    );

    let engine = engine.borrow();
    assert_eq!(engine.reads, 0);
    assert_eq!(engine.writes, 1);
}

#[test]
fn stale_cold_update_reads_only_after_failed_write() {
    let (mut storage, engine) = service();

    engine.borrow_mut().insert("records", "item", vec![2], 2);

    assert_eq!(
        handle_request(
            &mut storage,
            request(
                1,
                StorageOperation::Update {
                    namespace: "records".into(),
                    key: "item".into(),
                    content: vec![3],
                    expected_revision: 1,
                },
            ),
        )
        .outcome,
        StorageOutcome::Error(ExchangeError::RevisionConflict),
    );

    let engine = engine.borrow();
    assert_eq!(engine.writes, 1);
    assert_eq!(engine.reads, 1);
}

#[test]
fn missing_cold_update_reads_only_after_failed_write() {
    let (mut storage, engine) = service();

    assert_eq!(
        handle_request(
            &mut storage,
            request(
                1,
                StorageOperation::Update {
                    namespace: "records".into(),
                    key: "missing".into(),
                    content: vec![3],
                    expected_revision: 1,
                },
            ),
        )
        .outcome,
        StorageOutcome::Error(ExchangeError::NotFound),
    );

    let engine = engine.borrow();
    assert_eq!(engine.writes, 1);
    assert_eq!(engine.reads, 1);
}

#[test]
fn update_engine_failure_is_storage_failure() {
    let (mut storage, engine) = service();

    engine.borrow_mut().insert("records", "item", vec![1], 1);
    engine.borrow_mut().write_failures_remaining = 1;

    assert_eq!(
        handle_request(
            &mut storage,
            request(
                1,
                StorageOperation::Update {
                    namespace: "records".into(),
                    key: "item".into(),
                    content: vec![2],
                    expected_revision: 1,
                },
            ),
        )
        .outcome,
        StorageOutcome::Error(ExchangeError::StorageFailure),
    );

    let engine = engine.borrow();
    assert_eq!(engine.writes, 1);
    assert_eq!(engine.reads, 1);
}

#[test]
fn cached_stale_update_does_not_touch_engine() {
    let (mut storage, engine) = service();

    assert_eq!(
        handle_request(
            &mut storage,
            request(
                1,
                StorageOperation::Create {
                    namespace: "records".into(),
                    key: "item".into(),
                    content: vec![1],
                },
            ),
        )
        .outcome,
        StorageOutcome::Created { revision: 1 },
    );

    assert_eq!(
        handle_request(
            &mut storage,
            request(
                2,
                StorageOperation::Update {
                    namespace: "records".into(),
                    key: "item".into(),
                    content: vec![2],
                    expected_revision: 99,
                },
            ),
        )
        .outcome,
        StorageOutcome::Error(ExchangeError::RevisionConflict),
    );

    let engine = engine.borrow();
    assert_eq!(engine.reads, 0);
    assert_eq!(engine.writes, 1);
}

#[test]
fn successful_cold_delete_uses_one_engine_delete_and_no_read() {
    let (mut storage, engine) = service();

    engine.borrow_mut().insert("records", "item", vec![1], 1);

    assert_eq!(
        handle_request(
            &mut storage,
            request(
                1,
                StorageOperation::Delete {
                    namespace: "records".into(),
                    key: "item".into(),
                    expected_revision: 1,
                },
            ),
        )
        .outcome,
        StorageOutcome::Deleted,
    );

    let engine = engine.borrow();
    assert_eq!(engine.reads, 0);
    assert_eq!(engine.deletes, 1);
}

#[test]
fn stale_cold_delete_reads_only_after_failed_delete() {
    let (mut storage, engine) = service();

    engine.borrow_mut().insert("records", "item", vec![2], 2);

    assert_eq!(
        handle_request(
            &mut storage,
            request(
                1,
                StorageOperation::Delete {
                    namespace: "records".into(),
                    key: "item".into(),
                    expected_revision: 1,
                },
            ),
        )
        .outcome,
        StorageOutcome::Error(ExchangeError::RevisionConflict),
    );

    let engine = engine.borrow();
    assert_eq!(engine.deletes, 1);
    assert_eq!(engine.reads, 1);
}

#[test]
fn missing_cold_delete_reads_only_after_failed_delete() {
    let (mut storage, engine) = service();

    assert_eq!(
        handle_request(
            &mut storage,
            request(
                1,
                StorageOperation::Delete {
                    namespace: "records".into(),
                    key: "missing".into(),
                    expected_revision: 1,
                },
            ),
        )
        .outcome,
        StorageOutcome::Error(ExchangeError::NotFound),
    );

    let engine = engine.borrow();
    assert_eq!(engine.deletes, 1);
    assert_eq!(engine.reads, 1);
}

#[test]
fn delete_engine_failure_is_storage_failure() {
    let (mut storage, engine) = service();

    engine.borrow_mut().insert("records", "item", vec![1], 1);
    engine.borrow_mut().delete_failures_remaining = 1;

    assert_eq!(
        handle_request(
            &mut storage,
            request(
                1,
                StorageOperation::Delete {
                    namespace: "records".into(),
                    key: "item".into(),
                    expected_revision: 1,
                },
            ),
        )
        .outcome,
        StorageOutcome::Error(ExchangeError::StorageFailure),
    );

    let engine = engine.borrow();
    assert_eq!(engine.deletes, 1);
    assert_eq!(engine.reads, 0);
}

#[test]
fn cached_missing_delete_does_not_touch_engine_again() {
    let (mut storage, engine) = service();

    assert_eq!(
        handle_request(
            &mut storage,
            request(
                1,
                StorageOperation::Read {
                    namespace: "records".into(),
                    key: "missing".into(),
                },
            ),
        )
        .outcome,
        StorageOutcome::Error(ExchangeError::NotFound),
    );

    assert_eq!(
        handle_request(
            &mut storage,
            request(
                2,
                StorageOperation::Delete {
                    namespace: "records".into(),
                    key: "missing".into(),
                    expected_revision: 1,
                },
            ),
        )
        .outcome,
        StorageOutcome::Error(ExchangeError::NotFound),
    );

    let engine = engine.borrow();
    assert_eq!(engine.reads, 1);
    assert_eq!(engine.deletes, 0);
}

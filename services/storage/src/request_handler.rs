use storage_exchange_format::{
    ExchangeError, StorageOperation, StorageOutcome, StorageRequest, StorageResponse,
};

use crate::storage_engine::StorageEngine;
use crate::storage_service::StorageService;

pub fn handle_request<E: StorageEngine>(
    storage: &mut StorageService<E>,
    request: StorageRequest,
) -> StorageResponse {
    let outcome = execute(storage, request.operation).unwrap_or_else(StorageOutcome::Error);

    StorageResponse {
        id: request.id,
        outcome,
    }
}

fn execute<E: StorageEngine>(
    storage: &mut StorageService<E>,
    operation: StorageOperation,
) -> Result<StorageOutcome, ExchangeError> {
    match operation {
        StorageOperation::Ping => Ok(StorageOutcome::Pong),

        StorageOperation::Create {
            namespace,
            key,
            content,
        } => {
            validate(&namespace, &key)?;

            if storage
                .cached_state(&namespace, &key)
                .is_some_and(|record| record.is_some())
            {
                return Err(ExchangeError::AlreadyExists);
            }

            match storage.create(&namespace, &key, &content) {
                Ok(record) => Ok(StorageOutcome::Created {
                    revision: record.revision,
                }),
                Err(_) => match storage.refresh(&namespace, &key) {
                    Ok(Some(_)) => Err(ExchangeError::AlreadyExists),
                    Ok(None) | Err(_) => Err(ExchangeError::StorageFailure),
                },
            }
        }

        StorageOperation::Read { namespace, key } => {
            validate(&namespace, &key)?;

            let record = storage
                .read(&namespace, &key)
                .map_err(|_| ExchangeError::StorageFailure)?
                .ok_or(ExchangeError::NotFound)?;

            Ok(StorageOutcome::Data {
                content: record.content,
                revision: record.revision,
            })
        }

        StorageOperation::Update {
            namespace,
            key,
            content,
            expected_revision,
        } => {
            validate(&namespace, &key)?;

            if expected_revision <= 0 {
                return Err(ExchangeError::InvalidRequest);
            }

            if let Some(current) = storage.cached_state(&namespace, &key) {
                let current = current.ok_or(ExchangeError::NotFound)?;

                if current.revision != expected_revision {
                    return Err(ExchangeError::RevisionConflict);
                }
            }

            match storage.update(&namespace, &key, &content, expected_revision) {
                Ok(record) => Ok(StorageOutcome::Updated {
                    revision: record.revision,
                }),
                Err(_) => match storage.refresh(&namespace, &key) {
                    Ok(None) => Err(ExchangeError::NotFound),
                    Ok(Some(current)) if current.revision != expected_revision => {
                        Err(ExchangeError::RevisionConflict)
                    }
                    Ok(Some(_)) | Err(_) => Err(ExchangeError::StorageFailure),
                },
            }
        }

        StorageOperation::Delete {
            namespace,
            key,
            expected_revision,
        } => {
            validate(&namespace, &key)?;

            if expected_revision <= 0 {
                return Err(ExchangeError::InvalidRequest);
            }

            if let Some(current) = storage.cached_state(&namespace, &key) {
                let current = current.ok_or(ExchangeError::NotFound)?;

                if current.revision != expected_revision {
                    return Err(ExchangeError::RevisionConflict);
                }
            }

            match storage.delete(&namespace, &key, expected_revision) {
                Ok(true) => Ok(StorageOutcome::Deleted),
                Ok(false) => match storage.refresh(&namespace, &key) {
                    Ok(None) => Err(ExchangeError::NotFound),
                    Ok(Some(current)) if current.revision != expected_revision => {
                        Err(ExchangeError::RevisionConflict)
                    }
                    Ok(Some(_)) | Err(_) => Err(ExchangeError::StorageFailure),
                },
                Err(_) => Err(ExchangeError::StorageFailure),
            }
        }
    }
}

fn validate(namespace: &str, key: &str) -> Result<(), ExchangeError> {
    if namespace.is_empty() || key.is_empty() {
        return Err(ExchangeError::InvalidRequest);
    }

    Ok(())
}

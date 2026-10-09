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
                .read(&namespace, &key)
                .map_err(|_| ExchangeError::StorageFailure)?
                .is_some()
            {
                return Err(ExchangeError::AlreadyExists);
            }

            let record = storage
                .create(&namespace, &key, &content)
                .map_err(|_| ExchangeError::StorageFailure)?;

            Ok(StorageOutcome::Created {
                revision: record.revision,
            })
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

            let current = storage
                .read(&namespace, &key)
                .map_err(|_| ExchangeError::StorageFailure)?
                .ok_or(ExchangeError::NotFound)?;

            if current.revision != expected_revision {
                return Err(ExchangeError::RevisionConflict);
            }

            let record = storage
                .update(&namespace, &key, &content, expected_revision)
                .map_err(|_| ExchangeError::RevisionConflict)?;

            Ok(StorageOutcome::Updated {
                revision: record.revision,
            })
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

            let current = storage
                .read(&namespace, &key)
                .map_err(|_| ExchangeError::StorageFailure)?
                .ok_or(ExchangeError::NotFound)?;

            if current.revision != expected_revision {
                return Err(ExchangeError::RevisionConflict);
            }

            if !storage
                .delete(&namespace, &key, expected_revision)
                .map_err(|_| ExchangeError::StorageFailure)?
            {
                return Err(ExchangeError::RevisionConflict);
            }

            Ok(StorageOutcome::Deleted)
        }
    }
}

fn validate(namespace: &str, key: &str) -> Result<(), ExchangeError> {
    if namespace.is_empty() || key.is_empty() {
        return Err(ExchangeError::InvalidRequest);
    }

    Ok(())
}

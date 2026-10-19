use storage_exchange_format::{
    ExchangeError, StorageOperation, StorageOutcome, StorageRequest, StorageResponse,
};

use std::error::Error;

use crate::storage_engine::StorageEngine;
use crate::storage_mutation_error::StorageMutationError;
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

            let record = storage
                .create(&namespace, &key, &content)
                .map_err(map_error)?;

            Ok(StorageOutcome::Created {
                revision: record.revision,
            })
        }

        StorageOperation::Read { namespace, key } => {
            validate(&namespace, &key)?;

            let record = storage
                .read(&namespace, &key)
                .map_err(map_error)?
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
            validate_revision(expected_revision)?;

            let record = storage
                .update(&namespace, &key, &content, expected_revision)
                .map_err(map_error)?;

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
            validate_revision(expected_revision)?;

            storage
                .delete(&namespace, &key, expected_revision)
                .map_err(map_error)?;

            Ok(StorageOutcome::Deleted)
        }
    }
}

fn map_error(error: Box<dyn Error>) -> ExchangeError {
    match error.downcast_ref::<StorageMutationError>() {
        Some(StorageMutationError::AlreadyExists) => ExchangeError::AlreadyExists,
        Some(StorageMutationError::NotFound) => ExchangeError::NotFound,
        Some(StorageMutationError::RevisionConflict) => ExchangeError::RevisionConflict,
        None => ExchangeError::StorageFailure,
    }
}

fn validate(namespace: &str, key: &str) -> Result<(), ExchangeError> {
    if namespace.is_empty() || key.is_empty() {
        return Err(ExchangeError::InvalidRequest);
    }

    Ok(())
}

fn validate_revision(revision: i64) -> Result<(), ExchangeError> {
    if revision <= 0 {
        return Err(ExchangeError::InvalidRequest);
    }

    Ok(())
}

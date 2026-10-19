use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use crate::data_record::DataRecord;
use crate::storage_engine::{StorageEngine, StorageResult};
use crate::storage_mutation_error::StorageMutationError;

pub struct SqliteEngine {
    connection: Connection,
}

impl SqliteEngine {
    pub fn open(path: &Path) -> StorageResult<Self> {
        let connection = Connection::open(path)?;

        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS records (
                namespace TEXT NOT NULL,
                key TEXT NOT NULL,
                content BLOB NOT NULL,
                revision INTEGER NOT NULL CHECK (revision > 0),
                PRIMARY KEY (namespace, key)
            );
            ",
        )?;

        Ok(Self { connection })
    }

    fn revision(
        transaction: &Transaction<'_>,
        namespace: &str,
        key: &str,
    ) -> StorageResult<Option<i64>> {
        Ok(transaction
            .query_row(
                "SELECT revision FROM records WHERE namespace = ?1 AND key = ?2",
                params![namespace, key],
                |row| row.get(0),
            )
            .optional()?)
    }

    fn failed_revision(
        transaction: &Transaction<'_>,
        namespace: &str,
        key: &str,
    ) -> StorageResult<()> {
        match Self::revision(transaction, namespace, key)? {
            None => Err(StorageMutationError::NotFound.into()),
            Some(_) => Err(StorageMutationError::RevisionConflict.into()),
        }
    }
}

impl StorageEngine for SqliteEngine {
    fn read(&self, namespace: &str, key: &str) -> StorageResult<Option<DataRecord>> {
        Ok(self
            .connection
            .query_row(
                "
                SELECT namespace, key, content, revision
                FROM records
                WHERE namespace = ?1 AND key = ?2
                ",
                params![namespace, key],
                |row| {
                    Ok(DataRecord {
                        namespace: row.get(0)?,
                        key: row.get(1)?,
                        content: row.get(2)?,
                        revision: row.get(3)?,
                    })
                },
            )
            .optional()?)
    }

    fn write(
        &mut self,
        namespace: &str,
        key: &str,
        content: &[u8],
        expected_revision: Option<i64>,
    ) -> StorageResult<DataRecord> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;

        let revision = match expected_revision {
            None => {
                let inserted = transaction.execute(
                    "
                    INSERT INTO records (namespace, key, content, revision)
                    VALUES (?1, ?2, ?3, 1)
                    ON CONFLICT(namespace, key) DO NOTHING
                    ",
                    params![namespace, key, content],
                )?;

                if inserted != 1 {
                    return Err(StorageMutationError::AlreadyExists.into());
                }

                1
            }
            Some(previous) if previous > 0 => {
                let next = previous.checked_add(1).ok_or("revision overflow")?;

                let updated = transaction.execute(
                    "
                    UPDATE records
                    SET content = ?3, revision = ?4
                    WHERE namespace = ?1
                      AND key = ?2
                      AND revision = ?5
                    ",
                    params![namespace, key, content, next, previous],
                )?;

                if updated != 1 {
                    Self::failed_revision(&transaction, namespace, key)?;
                    return Err("unexpected update result".into());
                }

                next
            }
            _ => return Err("expected revision must be positive".into()),
        };

        transaction.commit()?;

        Ok(DataRecord {
            namespace: namespace.to_owned(),
            key: key.to_owned(),
            content: content.to_vec(),
            revision,
        })
    }

    fn delete(
        &mut self,
        namespace: &str,
        key: &str,
        expected_revision: i64,
    ) -> StorageResult<bool> {
        if expected_revision <= 0 {
            return Err("expected revision must be positive".into());
        }

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;

        let deleted = transaction.execute(
            "
            DELETE FROM records
            WHERE namespace = ?1
              AND key = ?2
              AND revision = ?3
            ",
            params![namespace, key, expected_revision],
        )?;

        if deleted != 1 {
            Self::failed_revision(&transaction, namespace, key)?;
            return Err("unexpected delete result".into());
        }

        transaction.commit()?;
        Ok(true)
    }
}

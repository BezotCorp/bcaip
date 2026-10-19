use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::data_record::DataRecord;
use crate::storage_engine::{StorageEngine, StorageResult};

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
}

impl StorageEngine for SqliteEngine {
    fn read(&self, namespace: &str, key: &str) -> StorageResult<Option<DataRecord>> {
        let record = self
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
            .optional()?;

        Ok(record)
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
                transaction.execute(
                    "
                    INSERT INTO records (namespace, key, content, revision)
                    VALUES (?1, ?2, ?3, 1)
                    ",
                    params![namespace, key, content],
                )?;

                1
            }
            Some(previous) if previous > 0 => {
                let updated = transaction.execute(
                    "
                    UPDATE records
                    SET content = ?3, revision = revision + 1
                    WHERE namespace = ?1
                      AND key = ?2
                      AND revision = ?4
                    ",
                    params![namespace, key, content, previous],
                )?;

                if updated != 1 {
                    return Err("Record not found or revision conflict".into());
                }

                previous.checked_add(1).ok_or("Revision overflow")?
            }
            _ => return Err("Expected revision must be positive".into()),
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
            return Err("Expected revision must be positive".into());
        }

        let deleted = self.connection.execute(
            "
            DELETE FROM records
            WHERE namespace = ?1
              AND key = ?2
              AND revision = ?3
            ",
            params![namespace, key, expected_revision],
        )?;

        Ok(deleted == 1)
    }
}

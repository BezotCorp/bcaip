use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub struct AtomicFile;

impl AtomicFile {
    pub fn write(target: &Path, bytes: &[u8]) -> Result<(), String> {
        let directory = target
            .parent()
            .ok_or_else(|| format!("{} has no parent directory", target.display()))?;

        fs::create_dir_all(directory).map_err(|error| {
            format!(
                "failed to create directory {}: {error}",
                directory.display()
            )
        })?;

        let file_name = target
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| format!("invalid filename: {}", target.display()))?;

        let temporary = Self::temporary_path(directory, file_name);

        let result = (|| -> Result<(), String> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|error| {
                    format!(
                        "failed to create temporary file {}: {error}",
                        temporary.display()
                    )
                })?;

            file.write_all(bytes)
                .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;

            file.sync_all()
                .map_err(|error| format!("failed to sync {}: {error}", temporary.display()))?;

            fs::rename(&temporary, target).map_err(|error| {
                format!(
                    "failed to replace {} from {}: {error}",
                    target.display(),
                    temporary.display()
                )
            })?;

            fs::File::open(directory)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| {
                    format!("failed to sync directory {}: {error}", directory.display())
                })?;

            Ok(())
        })();

        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }

        result
    }

    fn temporary_path(directory: &Path, file_name: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();

        let sequence = TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);

        directory.join(format!(
            ".{file_name}.{}.{}.{}.tmp",
            std::process::id(),
            timestamp,
            sequence
        ))
    }
}

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) struct AtomicFile;

impl AtomicFile {
    pub(crate) fn write(target: &Path, bytes: &[u8]) -> Result<(), String> {
        let directory = target
            .parent()
            .ok_or_else(|| format!("{} has no parent directory", target.display()))?;
        let file_name = target
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| format!("invalid file name: {}", target.display()))?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temp = directory.join(format!(".{file_name}.{}.{stamp}.tmp", std::process::id()));

        let result = (|| -> Result<(), String> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
                .map_err(|error| format!("failed to create {}: {error}", temp.display()))?;

            file.write_all(bytes)
                .map_err(|error| format!("failed to write {}: {error}", temp.display()))?;
            file.sync_all()
                .map_err(|error| format!("failed to sync {}: {error}", temp.display()))?;
            fs::rename(&temp, target)
                .map_err(|error| format!("failed to replace {}: {error}", target.display()))?;
            fs::File::open(directory)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| {
                    format!("failed to sync directory {}: {error}", directory.display())
                })?;

            Ok(())
        })();

        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }

        result
    }
}

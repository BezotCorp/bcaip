use crate::update_record::UpdateRecord;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub(crate) struct UpdateStateStore {
    directory: PathBuf,
}

impl UpdateStateStore {
    pub(crate) fn new(directory: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&directory).map_err(|error| {
            format!(
                "failed to create watcher state directory {}: {error}",
                directory.display()
            )
        })?;

        Ok(Self { directory })
    }

    pub(crate) fn read(&self, product: &str) -> Result<Option<UpdateRecord>, String> {
        let path = self.path_for(product);

        if !path.exists() {
            return Ok(None);
        }

        let source = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

        let record: UpdateRecord = ron::from_str(&source)
            .map_err(|error| format!("failed to decode {}: {error}", path.display()))?;

        if record.product != product {
            return Err(format!(
                "watcher state {} belongs to {}, expected {}",
                path.display(),
                record.product,
                product
            ));
        }

        Ok(Some(record))
    }

    pub(crate) fn write(&self, record: &UpdateRecord) -> Result<(), String> {
        let target = self.path_for(&record.product);

        let serialized = ron::ser::to_string(record)
            .map_err(|error| format!("failed to encode watcher state: {error}"))?;

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();

        let temp = self.directory.join(format!(
            ".{}.{}.{}.tmp",
            record.product,
            std::process::id(),
            stamp
        ));

        let result = (|| -> Result<(), String> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
                .map_err(|error| {
                    format!("failed to create {}: {error}", temp.display())
                })?;

            file.write_all(serialized.as_bytes()).map_err(|error| {
                format!("failed to write {}: {error}", temp.display())
            })?;

            file.sync_all().map_err(|error| {
                format!("failed to sync {}: {error}", temp.display())
            })?;

            fs::rename(&temp, &target).map_err(|error| {
                format!(
                    "failed to replace watcher state {}: {error}",
                    target.display()
                )
            })?;

            sync_directory(&self.directory)?;

            Ok(())
        })();

        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }

        result
    }

    fn path_for(&self, product: &str) -> PathBuf {
        self.directory.join(format!("{product}.ron"))
    }
}

fn sync_directory(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            format!(
                "failed to sync directory {}: {error}",
                path.display()
            )
        })
}

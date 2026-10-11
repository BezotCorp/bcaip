use crate::atomic_file::AtomicFile;
use std::fs;
use std::path::{Path, PathBuf};

/// Snapshot of the runtime state file taken before a candidate may run.
/// It is also copied next to the original so it survives a supervisor crash.
pub(crate) struct RuntimeStateBackup {
    path: PathBuf,
    copy_path: PathBuf,
    bytes: Option<Vec<u8>>,
}

impl RuntimeStateBackup {
    pub(crate) fn capture(path: &Path) -> Result<Self, String> {
        let copy_path = Self::copy_path_of(path);

        let bytes = if path.exists() {
            let bytes = fs::read(path)
                .map_err(|error| format!("failed to backup {}: {error}", path.display()))?;

            AtomicFile::write(&copy_path, &bytes)?;

            Some(bytes)
        } else {
            if copy_path.exists() {
                fs::remove_file(&copy_path).map_err(|error| {
                    format!("failed to remove stale {}: {error}", copy_path.display())
                })?;
            }

            None
        };

        Ok(Self {
            path: path.to_path_buf(),
            copy_path,
            bytes,
        })
    }

    fn copy_path_of(path: &Path) -> PathBuf {
        let mut name = path.as_os_str().to_os_string();
        name.push(".pre-update");

        PathBuf::from(name)
    }

    /// Restores the runtime state from its durable copy; no copy means the
    /// state did not exist when the backup was taken.
    pub(crate) fn restore_saved(path: &Path) -> Result<(), String> {
        let copy_path = Self::copy_path_of(path);

        if copy_path.exists() {
            let bytes = fs::read(&copy_path)
                .map_err(|error| format!("failed to read {}: {error}", copy_path.display()))?;

            return AtomicFile::write(path, &bytes);
        }

        if path.exists() {
            fs::remove_file(path)
                .map_err(|error| format!("failed to remove {}: {error}", path.display()))?;
        }

        Ok(())
    }

    pub(crate) fn copy_path(&self) -> &Path {
        &self.copy_path
    }

    pub(crate) fn matches_current(&self) -> Result<bool, String> {
        match &self.bytes {
            Some(expected) => {
                let current = fs::read(&self.path)
                    .map_err(|error| format!("failed to read {}: {error}", self.path.display()))?;

                Ok(&current == expected)
            }

            None => Ok(!self.path.exists()),
        }
    }

    pub(crate) fn restore(&self) -> Result<(), String> {
        match &self.bytes {
            Some(bytes) => AtomicFile::write(&self.path, bytes),

            None => {
                if self.path.exists() {
                    fs::remove_file(&self.path).map_err(|error| {
                        format!("failed to remove {}: {error}", self.path.display())
                    })?;
                }

                Ok(())
            }
        }
    }
}

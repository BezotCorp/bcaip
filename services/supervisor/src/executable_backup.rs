use std::fs;
use std::path::{Path, PathBuf};

/// Previous binary of an executable, kept as a hard link so that the old
/// inode survives the atomic replacement performed by an update.
pub(crate) struct ExecutableBackup {
    target: PathBuf,
    previous: PathBuf,
}

impl ExecutableBackup {
    pub(crate) fn of(target: &Path) -> Result<Self, String> {
        let parent = target
            .parent()
            .ok_or_else(|| format!("{} has no parent directory", target.display()))?;
        let file_name = target
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| format!("invalid executable filename: {}", target.display()))?;

        Ok(Self {
            target: target.to_path_buf(),
            previous: parent.join(format!(".{file_name}.previous")),
        })
    }

    pub(crate) fn prepare(&self) -> Result<(), String> {
        if self.previous.exists() {
            fs::remove_file(&self.previous).map_err(|error| {
                format!("failed to remove {}: {error}", self.previous.display())
            })?;
        }

        fs::hard_link(&self.target, &self.previous).map_err(|error| {
            format!(
                "failed to back up {} to {}: {error}",
                self.target.display(),
                self.previous.display()
            )
        })
    }

    pub(crate) fn is_available(&self) -> bool {
        self.previous.is_file()
    }

    pub(crate) fn restore(&self) -> Result<(), String> {
        if !self.previous.is_file() {
            return Err(format!(
                "no previous binary to restore at {}",
                self.previous.display()
            ));
        }

        fs::rename(&self.previous, &self.target).map_err(|error| {
            format!(
                "failed to restore {} from {}: {error}",
                self.target.display(),
                self.previous.display()
            )
        })?;

        let parent = self
            .target
            .parent()
            .ok_or_else(|| format!("{} has no parent directory", self.target.display()))?;

        fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("failed to sync directory {}: {error}", parent.display()))
    }
}

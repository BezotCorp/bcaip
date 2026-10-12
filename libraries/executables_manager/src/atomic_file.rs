use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) struct AtomicFile;

impl AtomicFile {
    pub(crate) fn write(path: &Path, content: &[u8]) -> Result<(), String> {
        let parent = path
            .parent()
            .ok_or_else(|| format!("{} has no parent directory", path.display()))?;

        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;

        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| format!("invalid filename: {}", path.display()))?;

        let temporary: PathBuf = parent.join(format!(".{file_name}.tmp"));

        fs::write(&temporary, content)
            .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;

        fs::rename(&temporary, path).map_err(|error| {
            format!(
                "failed to replace {} from {}: {error}",
                path.display(),
                temporary.display()
            )
        })
    }
}

use std::{fs, path::Path};

pub(crate) struct FileWriter;

impl FileWriter {
    pub(crate) fn write(path: &Path, content: &[u8]) -> Result<(), String> {
        let parent = path
            .parent()
            .ok_or_else(|| format!("{} has no parent directory", path.display()))?;

        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;

        let filename = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| format!("invalid filename: {}", path.display()))?;

        let temporary = parent.join(format!(".{filename}.tmp"));

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

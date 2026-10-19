use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

pub struct UpdateFile;

impl UpdateFile {
    pub fn temporary_path(target: &Path, update_id: &str) -> Result<PathBuf, String> {
        let parent = target
            .parent()
            .ok_or_else(|| format!("{} has no parent directory", target.display()))?;

        let file_name = target
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| format!("invalid executable filename: {}", target.display()))?;

        Ok(parent.join(format!(".{file_name}.{update_id}.update.tmp")))
    }

    pub fn matches(path: &Path, expected_size: u64, expected_sha256: &str) -> Result<bool, String> {
        if !path.is_file() {
            return Ok(false);
        }

        let mut file = fs::File::open(path)
            .map_err(|error| format!("failed to open {}: {error}", path.display()))?;

        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        let mut size = 0_u64;

        loop {
            let read = file
                .read(&mut buffer)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

            if read == 0 {
                break;
            }

            size = size
                .checked_add(read as u64)
                .ok_or_else(|| "file size overflow".to_string())?;

            hasher.update(&buffer[..read]);
        }

        let digest = hasher.finalize();

        let actual_sha256 = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();

        Ok(size == expected_size && actual_sha256.eq_ignore_ascii_case(expected_sha256))
    }

    pub fn sync_directory(path: &Path) -> Result<(), String> {
        fs::File::open(path)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("failed to sync directory {}: {error}", path.display()))
    }
}

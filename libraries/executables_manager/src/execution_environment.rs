use std::{
    fs,
    path::{Path, PathBuf},
};

const PRODUCTION_LOCK: &str = "production.lock";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionEnvironment {
    Development,
    Production,
}

impl ExecutionEnvironment {
    pub fn detect(data_directory: &Path) -> Result<Self, String> {
        let lock = Self::production_lock_path(data_directory);

        let metadata = match fs::metadata(&lock) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::Development);
            }
            Err(error) => {
                return Err(format!("failed to inspect {}: {error}", lock.display()));
            }
        };

        if !metadata.is_file() || metadata.len() != 0 {
            return Err(format!("{} must be an empty regular file", lock.display()));
        }

        Ok(Self::Production)
    }

    pub fn production_lock_path(data_directory: &Path) -> PathBuf {
        data_directory.join(PRODUCTION_LOCK)
    }

    pub fn production_lock_file_name() -> &'static str {
        PRODUCTION_LOCK
    }

    pub fn resolve(self, path: &Path) -> PathBuf {
        if self == Self::Production {
            return path.to_path_buf();
        }

        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            return path.to_path_buf();
        };

        let local = match file_name.rsplit_once('.') {
            Some((stem, extension)) if !stem.is_empty() => {
                format!("{stem}.local.{extension}")
            }
            _ => format!("{file_name}.local"),
        };

        path.with_file_name(local)
    }
}

use std::path::{Path, PathBuf};

use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Decode, Deserialize, Encode, Serialize)]
pub struct ServerPaths {
    temporary_root_path: String,
    executable_root_path: String,
    data_root_path: String,
}

impl ServerPaths {
    pub fn new(
        temporary_root_path: String,
        executable_root_path: String,
        data_root_path: String,
    ) -> Result<Self, String> {
        let paths = Self {
            temporary_root_path,
            executable_root_path,
            data_root_path,
        };

        paths.validate()?;

        Ok(paths)
    }

    pub fn temporary_root_path(&self) -> &str {
        &self.temporary_root_path
    }

    pub fn executable_root_path(&self) -> &str {
        &self.executable_root_path
    }

    pub fn data_root_path(&self) -> &str {
        &self.data_root_path
    }

    pub fn temporary_root(&self) -> PathBuf {
        PathBuf::from(&self.temporary_root_path)
    }

    pub fn executable_root(&self) -> PathBuf {
        PathBuf::from(&self.executable_root_path)
    }

    pub fn data_root(&self) -> PathBuf {
        PathBuf::from(&self.data_root_path)
    }

    pub fn validate(&self) -> Result<(), String> {
        Self::validate_absolute("temporary_root_path", &self.temporary_root_path)?;
        Self::validate_absolute("executable_root_path", &self.executable_root_path)?;
        Self::validate_absolute("data_root_path", &self.data_root_path)?;

        Ok(())
    }

    fn validate_absolute(name: &str, value: &str) -> Result<(), String> {
        if value.trim().is_empty() {
            return Err(format!("{name} must not be empty"));
        }

        let path = Path::new(value);

        if !path.is_absolute() {
            return Err(format!("{name} must be absolute: {value}"));
        }

        Ok(())
    }
}

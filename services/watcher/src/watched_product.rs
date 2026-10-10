use serde::Deserialize;
use std::path::{Component, Path, PathBuf};

#[derive(Deserialize, Clone, Debug)]
pub(crate) struct WatchedProduct {
    pub(crate) name: String,
    pub(crate) executable_path: String,
}

impl WatchedProduct {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.name.is_empty()
            || !self
                .name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err(format!("invalid product name: {}", self.name));
        }

        let path = Path::new(&self.executable_path);

        if self.executable_path.trim().is_empty() {
            return Err(format!(
                "{} executable_path must not be empty",
                self.name
            ));
        }

        if !path.is_absolute() {
            return Err(format!(
                "{} executable_path must be absolute",
                self.name
            ));
        }

        for component in path.components() {
            if matches!(component, Component::ParentDir | Component::CurDir) {
                return Err(format!(
                    "{} executable_path must not contain relative segments",
                    self.name
                ));
            }
        }

        Ok(())
    }

    pub(crate) fn executable_path(&self) -> PathBuf {
        PathBuf::from(&self.executable_path)
    }
}

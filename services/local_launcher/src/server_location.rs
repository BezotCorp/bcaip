use bincode_next::{Decode, Encode};
use serde::Deserialize;
use std::path::{Component, Path, PathBuf};

#[derive(Encode, Decode, Deserialize, Clone, Debug)]
pub(crate) enum ServerLocation {
    Local {
        root_path: String,
    },
}

impl ServerLocation {
    pub(crate) fn validate(&self) -> Result<(), String> {
        match self {
            Self::Local { root_path } => validate_root_path(root_path),
        }
    }

    pub(crate) fn root_path(&self) -> &Path {
        match self {
            Self::Local { root_path } => Path::new(root_path),
        }
    }

    pub(crate) fn executable_path(&self, relative_path: &Path) -> PathBuf {
        self.root_path().join(relative_path)
    }
}

fn validate_root_path(value: &str) -> Result<(), String> {
    let path = Path::new(value);

    if value.trim().is_empty() {
        return Err("local server root_path must not be empty".to_string());
    }

    if !path.is_absolute() {
        return Err("local server root_path must be absolute".to_string());
    }

    for component in path.components() {
        if matches!(component, Component::ParentDir | Component::CurDir) {
            return Err(
                "local server root_path must not contain relative segments".to_string(),
            );
        }
    }

    Ok(())
}

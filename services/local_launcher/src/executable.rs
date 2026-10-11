use crate::process::Process;
use crate::update::Update;
use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};

#[derive(Encode, Decode, Deserialize, Serialize, Clone, Debug)]
pub(crate) struct Executable {
    pub(crate) name: String,
    pub(crate) working_directory: String,
    pub(crate) executable_path: String,
    pub(crate) default_ws_address: SocketAddr,

    #[serde(default)]
    pub(crate) process: Option<Process>,

    #[serde(default)]
    pub(crate) update: Option<Update>,
}

impl Executable {
    pub(crate) fn validate_definition(&self) -> Result<(), String> {
        if self.name.is_empty()
            || !self
                .name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        {
            return Err(format!("invalid executable name: {}", self.name));
        }

        let working_directory = Path::new(&self.working_directory);

        if self.working_directory.trim().is_empty() {
            return Err(format!("{} working_directory must not be empty", self.name));
        }

        if !working_directory.is_absolute() {
            return Err(format!("{} working_directory must be absolute", self.name));
        }

        for component in working_directory.components() {
            if matches!(component, Component::ParentDir | Component::CurDir) {
                return Err(format!(
                    "{} working_directory must not contain relative segments",
                    self.name
                ));
            }
        }

        let executable_path = Path::new(&self.executable_path);

        if self.executable_path.trim().is_empty() {
            return Err(format!("{} executable_path must not be empty", self.name));
        }

        if executable_path.is_absolute() {
            return Err(format!(
                "{} executable_path must be relative to working_directory",
                self.name
            ));
        }

        for component in executable_path.components() {
            if matches!(
                component,
                Component::ParentDir
                    | Component::RootDir
                    | Component::Prefix(_)
                    | Component::CurDir
            ) {
                return Err(format!(
                    "{} executable_path must stay inside working_directory",
                    self.name
                ));
            }
        }

        Ok(())
    }

    pub(crate) fn path(&self) -> PathBuf {
        Path::new(&self.working_directory).join(&self.executable_path)
    }
}

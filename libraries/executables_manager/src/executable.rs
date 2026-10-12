use std::{
    net::SocketAddr,
    path::{Component, Path, PathBuf},
};

use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{
    executable_id::ExecutableId, executable_name::ExecutableName,
    executable_start_phase::ExecutableStartPhase,
};

#[derive(Clone, Debug, Decode, Deserialize, Encode, Serialize)]
pub struct Executable {
    id: ExecutableId,
    name: ExecutableName,
    start_phase: ExecutableStartPhase,
    working_directory: String,
    executable_path: String,
    default_ws_address: SocketAddr,
}

impl Executable {
    pub fn new(
        id: ExecutableId,
        name: ExecutableName,
        start_phase: ExecutableStartPhase,
        working_directory: String,
        executable_path: String,
        default_ws_address: SocketAddr,
    ) -> Result<Self, String> {
        let executable = Self {
            id,
            name,
            start_phase,
            working_directory,
            executable_path,
            default_ws_address,
        };

        executable.validate()?;

        Ok(executable)
    }

    pub fn id(&self) -> ExecutableId {
        self.id
    }

    pub fn name(&self) -> &ExecutableName {
        &self.name
    }

    pub fn start_phase(&self) -> ExecutableStartPhase {
        self.start_phase
    }

    pub fn working_directory(&self) -> &str {
        &self.working_directory
    }

    pub fn executable_path(&self) -> &str {
        &self.executable_path
    }

    pub fn default_ws_address(&self) -> SocketAddr {
        self.default_ws_address
    }

    pub fn path(&self) -> PathBuf {
        Path::new(&self.working_directory).join(&self.executable_path)
    }

    pub fn validate(&self) -> Result<(), String> {
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
}

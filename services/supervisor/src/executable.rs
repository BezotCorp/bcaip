use crate::process::Process;
use crate::update::Update;
use bincode_next::{Decode, Encode, encode_to_vec};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};

#[derive(Encode, Decode, Deserialize, Serialize, Clone, Debug)]
pub(crate) struct Executable {
    name: String,
    working_directory: String,
    executable_path: String,
    default_ws_address: SocketAddr,

    #[serde(default)]
    process: Option<Process>,

    #[serde(default)]
    update: Option<Update>,
}

impl Executable {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn name_owned(&self) -> String {
        self.name.clone()
    }

    pub(crate) fn working_directory(&self) -> &str {
        &self.working_directory
    }

    pub(crate) fn default_ws_address(&self) -> SocketAddr {
        self.default_ws_address
    }

    pub(crate) fn process(&self) -> Option<&Process> {
        self.process.as_ref()
    }

    pub(crate) fn update(&self) -> Option<&Update> {
        self.update.as_ref()
    }

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

    pub(crate) fn is_named(&self, name: &str) -> bool {
        self.name == name
    }

    pub(crate) fn has_runtime_state(&self) -> bool {
        self.process.is_some() || self.update.is_some()
    }

    pub(crate) fn process_instance_id(&self) -> Option<&str> {
        self.process.as_ref().map(Process::instance_id)
    }

    pub(crate) fn process_instance_id_owned(&self) -> Option<String> {
        self.process.as_ref().map(Process::instance_id_owned)
    }

    pub(crate) fn committed_update_id(&self) -> Option<&str> {
        self.update.as_ref().and_then(Update::committed_update_id)
    }

    pub(crate) fn needs_update_restart(&self) -> bool {
        let Some(update_id) = self.committed_update_id() else {
            return false;
        };

        let Some(process) = self.process.as_ref() else {
            return false;
        };

        !process.has_applied_update_id(update_id)
    }

    pub(crate) fn encode_runtime_state(&self) -> Result<Vec<u8>, String> {
        encode_to_vec(
            (&self.name, &self.process, &self.update),
            bincode_next::config::standard(),
        )
        .map_err(|error| format!("failed to encode runtime state of {}: {error}", self.name))
    }

    pub(crate) fn record_started_process(&mut self, process: Process) {
        self.process = Some(process);
    }

    pub(crate) fn remove_process(&mut self) -> Option<Process> {
        self.process.take()
    }

    pub(crate) fn recover_process_after_persist_failure(&mut self, process: Option<Process>) {
        self.process = process;
    }

    pub(crate) fn has_process_instance(&self, instance_id: &str) -> bool {
        self.process
            .as_ref()
            .is_some_and(|process| process.has_instance_id(instance_id))
    }

    pub(crate) fn replace_update_state(&mut self, update: Update) {
        self.update = Some(update);
    }

    pub(crate) fn update_owned(&self) -> Option<Update> {
        self.update.clone()
    }

    pub(crate) fn recover_update_after_persist_failure(&mut self, update: Option<Update>) {
        self.update = update;
    }

    pub(crate) fn has_same_materialization(&self, other: &Executable) -> bool {
        self.working_directory == other.working_directory
            && self.executable_path == other.executable_path
    }

    pub(crate) fn merge_runtime_from(&mut self, previous: &Executable) {
        self.process = previous.process.clone();
        self.update = previous.update.clone();
    }
}

use std::{
    net::SocketAddr,
    path::{Component, Path, PathBuf},
};

use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{
    ExecutableId, ExecutableName, ExecutableRole, ExecutableRuntime, ExecutableStartPhase, Process,
    Update,
};

#[derive(Clone, Debug, Decode, Deserialize, Encode, Serialize)]
pub struct Executable {
    id: ExecutableId,
    name: ExecutableName,
    start_phase: ExecutableStartPhase,
    #[serde(default)]
    roles: Vec<ExecutableRole>,
    working_directory: String,
    executable_path: String,
    default_ws_address: SocketAddr,
    #[serde(default)]
    process: Option<Process>,
    #[serde(default)]
    update: Option<Update>,
}

impl Executable {
    pub fn new(
        id: ExecutableId,
        name: ExecutableName,
        start_phase: ExecutableStartPhase,
        roles: Vec<ExecutableRole>,
        working_directory: String,
        executable_path: String,
        default_ws_address: SocketAddr,
    ) -> Result<Self, String> {
        let executable = Self {
            id,
            name,
            start_phase,
            roles,
            working_directory,
            executable_path,
            default_ws_address,
            process: None,
            update: None,
        };

        executable.validate()?;

        Ok(executable)
    }

    pub fn with_runtime(mut self, runtime: ExecutableRuntime) -> Self {
        let (process, update) = runtime.into_state();

        self.process = process;
        self.update = update;

        self
    }

    pub fn runtime(&self) -> ExecutableRuntime {
        ExecutableRuntime::new(self.id, self.process.clone(), self.update.clone())
    }

    pub fn id(&self) -> ExecutableId {
        self.id
    }

    pub fn name(&self) -> &ExecutableName {
        &self.name
    }

    pub fn name_owned(&self) -> String {
        self.name.as_str().to_string()
    }

    pub fn is_named(&self, name: &str) -> bool {
        self.name.as_str() == name
    }

    pub fn start_phase(&self) -> ExecutableStartPhase {
        self.start_phase
    }

    pub fn starts_at_runtime(&self) -> bool {
        self.start_phase == ExecutableStartPhase::Runtime
    }

    pub fn roles(&self) -> &[ExecutableRole] {
        &self.roles
    }

    pub fn has_role(&self, role: ExecutableRole) -> bool {
        self.roles.contains(&role)
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

    pub fn working_directory_path(&self, executable_root: &Path) -> PathBuf {
        executable_root.join(&self.working_directory)
    }

    pub fn path(&self, executable_root: &Path) -> PathBuf {
        self.working_directory_path(executable_root)
            .join(&self.executable_path)
    }

    pub fn process(&self) -> Option<&Process> {
        self.process.as_ref()
    }

    pub fn update(&self) -> Option<&Update> {
        self.update.as_ref()
    }

    pub fn process_instance_id_owned(&self) -> Option<String> {
        self.process.as_ref().map(Process::instance_id_owned)
    }

    pub fn committed_update_id(&self) -> Option<&str> {
        self.update.as_ref().and_then(Update::committed_update_id)
    }

    pub fn needs_update_restart(&self) -> bool {
        let Some(update_id) = self.committed_update_id() else {
            return false;
        };

        let Some(process) = self.process.as_ref() else {
            return false;
        };

        !process.has_applied_update_id(update_id)
    }

    pub fn record_started_process(&mut self, process: Process) {
        self.process = Some(process);
    }

    pub fn remove_process(&mut self) -> Option<Process> {
        self.process.take()
    }

    pub fn recover_process_after_persist_failure(&mut self, process: Option<Process>) {
        self.process = process;
    }

    pub fn has_process_instance(&self, instance_id: &str) -> bool {
        self.process
            .as_ref()
            .is_some_and(|process| process.has_instance_id(instance_id))
    }

    pub fn replace_update_state(&mut self, update: Update) {
        self.update = Some(update);
    }

    pub fn update_owned(&self) -> Option<Update> {
        self.update.clone()
    }

    pub fn recover_update_after_persist_failure(&mut self, update: Option<Update>) {
        self.update = update;
    }

    pub fn validate(&self) -> Result<(), String> {
        let working_directory = Path::new(&self.working_directory);

        if working_directory.is_absolute() {
            return Err(format!(
                "{} working_directory must be relative to executable_root",
                self.name
            ));
        }

        for component in working_directory.components() {
            if matches!(
                component,
                Component::ParentDir
                    | Component::RootDir
                    | Component::Prefix(_)
                    | Component::CurDir
            ) {
                return Err(format!(
                    "{} working_directory must stay inside executable_root",
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

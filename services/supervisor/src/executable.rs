use executables_manager::{Executable as ExecutableDefinition, ExecutableId, ExecutableStartPhase};

use crate::executable_runtime::ExecutableRuntime;
use executables_manager::{Process, Update};

/// In-memory association of a definition (from `executables_manager`) with
/// the supervisor's runtime state of the same `ExecutableId`.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct Executable {
    definition: ExecutableDefinition,
    process: Option<Process>,
    update: Option<Update>,
}

impl Executable {
    pub(crate) fn new(definition: ExecutableDefinition) -> Self {
        Self {
            definition,
            process: None,
            update: None,
        }
    }

    pub(crate) fn with_runtime(
        definition: ExecutableDefinition,
        runtime: ExecutableRuntime,
    ) -> Self {
        let (process, update) = runtime.into_state();

        Self {
            definition,
            process,
            update,
        }
    }

    pub(crate) fn runtime(&self) -> ExecutableRuntime {
        ExecutableRuntime::new(self.id(), self.process.clone(), self.update.clone())
    }

    pub(crate) fn starts_at_runtime(&self) -> bool {
        self.start_phase() == ExecutableStartPhase::Runtime
    }

    pub(crate) fn id(&self) -> ExecutableId {
        self.definition.id()
    }

    pub(crate) fn name(&self) -> &str {
        self.definition.name().as_str()
    }

    pub(crate) fn name_owned(&self) -> String {
        self.name().to_string()
    }

    pub(crate) fn start_phase(&self) -> ExecutableStartPhase {
        self.definition.start_phase()
    }

    pub(crate) fn working_directory(&self) -> &str {
        self.definition.working_directory()
    }

    pub(crate) fn default_ws_address(&self) -> std::net::SocketAddr {
        self.definition.default_ws_address()
    }

    pub(crate) fn path(&self) -> std::path::PathBuf {
        self.definition.path()
    }

    pub(crate) fn process(&self) -> Option<&Process> {
        self.process.as_ref()
    }

    pub(crate) fn update(&self) -> Option<&Update> {
        self.update.as_ref()
    }

    pub(crate) fn validate_definition(&self) -> Result<(), String> {
        self.definition.validate()
    }

    pub(crate) fn is_named(&self, name: &str) -> bool {
        self.name() == name
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
}

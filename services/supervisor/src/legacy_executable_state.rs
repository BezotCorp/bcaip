use crate::executable_runtime::ExecutableRuntime;
use bincode_next::Decode;
use executables_manager::Process;
use executables_manager::Update;
use executables_manager::{ExecutableId, ExecutableName, ExecutableStartPhase};
use std::net::SocketAddr;

/// Former content of `executables.bin`: one record per executable embedding
/// its definition. bincode writes structs field after field with no framing,
/// so the definition fields are flattened here and decode identically to the
/// nested layout. Only the identity and the runtime state survive a migration.
#[derive(Decode)]
pub(crate) struct LegacyExecutableState {
    id: ExecutableId,
    _name: ExecutableName,
    _start_phase: ExecutableStartPhase,
    _working_directory: String,
    _executable_path: String,
    _default_ws_address: SocketAddr,
    process: Option<Process>,
    update: Option<Update>,
}

impl LegacyExecutableState {
    pub(crate) fn into_runtime(self) -> ExecutableRuntime {
        ExecutableRuntime::new(self.id, self.process, self.update)
    }
}

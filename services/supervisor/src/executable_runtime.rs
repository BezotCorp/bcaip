use crate::legacy_executable_state::LegacyExecutableState;
use bincode_next::{Decode, Encode, decode_from_slice, encode_to_vec};
use executables_manager::ExecutableId;
use executables_manager::Process;
use executables_manager::Update;
use serde::{Deserialize, Serialize};

/// The only thing the supervisor persists about an executable: the mutable
/// state it owns, keyed by the stable `ExecutableId`. The definition itself
/// always comes from `executables_manager`.
#[derive(Clone, Debug, Decode, Deserialize, Encode, Serialize)]
pub(crate) struct ExecutableRuntime {
    executable_id: ExecutableId,
    process: Option<Process>,
    update: Option<Update>,
}

impl ExecutableRuntime {
    pub(crate) fn new(
        executable_id: ExecutableId,
        process: Option<Process>,
        update: Option<Update>,
    ) -> Self {
        Self {
            executable_id,
            process,
            update,
        }
    }

    pub(crate) fn executable_id(&self) -> ExecutableId {
        self.executable_id
    }

    pub(crate) fn process(&self) -> Option<&Process> {
        self.process.as_ref()
    }

    pub(crate) fn update(&self) -> Option<&Update> {
        self.update.as_ref()
    }

    pub(crate) fn without_process(mut self) -> Self {
        self.process = None;
        self
    }

    pub(crate) fn into_state(self) -> (Option<Process>, Option<Update>) {
        (self.process, self.update)
    }

    pub(crate) fn encode(&self) -> Result<Vec<u8>, String> {
        encode_to_vec(self, bincode_next::config::standard()).map_err(|error| {
            format!(
                "failed to encode runtime state of {}: {error}",
                self.executable_id
            )
        })
    }

    /// The one reader of persisted runtime state (the state file and its
    /// backups): the current layout, or the former one migrated to it. The
    /// flag tells whether a migration happened. Nothing is written here.
    pub(crate) fn decode_state(bytes: &[u8]) -> Result<(Vec<Self>, bool), String> {
        let config = bincode_next::config::standard();

        if let Ok((runtimes, consumed)) = decode_from_slice::<Vec<Self>, _>(bytes, config)
            && consumed == bytes.len()
        {
            return Ok((runtimes, false));
        }

        if let Ok((legacy, consumed)) =
            decode_from_slice::<Vec<LegacyExecutableState>, _>(bytes, config)
            && consumed == bytes.len()
        {
            return Ok((
                legacy
                    .into_iter()
                    .map(LegacyExecutableState::into_runtime)
                    .collect(),
                true,
            ));
        }

        Err("unknown runtime state format".to_string())
    }

    pub(crate) fn encode_state(runtimes: &[Self]) -> Result<Vec<u8>, String> {
        encode_to_vec(runtimes, bincode_next::config::standard())
            .map_err(|error| format!("failed to encode executables runtime: {error}"))
    }
}

use crate::{ExecutableId, Process, Update};
use bincode_next::{Decode, Encode, decode_from_slice, encode_to_vec};
use serde::{Deserialize, Serialize};

/// Mutable runtime state associated with an executable, keyed by its stable
/// `ExecutableId`. The executable definition is stored separately.
#[derive(Clone, Debug, Decode, Deserialize, Encode, Serialize)]
pub struct ExecutableRuntime {
    executable_id: ExecutableId,
    process: Option<Process>,
    update: Option<Update>,
}

impl ExecutableRuntime {
    pub fn new(
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

    pub fn executable_id(&self) -> ExecutableId {
        self.executable_id
    }

    pub fn process(&self) -> Option<&Process> {
        self.process.as_ref()
    }

    pub fn update(&self) -> Option<&Update> {
        self.update.as_ref()
    }

    pub fn without_process(mut self) -> Self {
        self.process = None;
        self
    }

    pub fn into_state(self) -> (Option<Process>, Option<Update>) {
        (self.process, self.update)
    }

    pub fn encode(&self) -> Result<Vec<u8>, String> {
        encode_to_vec(self, bincode_next::config::standard()).map_err(|error| {
            format!(
                "failed to encode runtime state of {}: {error}",
                self.executable_id
            )
        })
    }

    /// Decodes persisted executable runtime state using the current format.
    pub fn decode_state(bytes: &[u8]) -> Result<Vec<Self>, String> {
        let config = bincode_next::config::standard();

        let (runtimes, consumed) = decode_from_slice::<Vec<Self>, _>(bytes, config)
            .map_err(|error| format!("failed to decode executable runtime state: {error}"))?;

        if consumed != bytes.len() {
            return Err("executable runtime state contains trailing bytes".to_string());
        }

        Ok(runtimes)
    }

    pub fn encode_state(runtimes: &[Self]) -> Result<Vec<u8>, String> {
        encode_to_vec(runtimes, bincode_next::config::standard())
            .map_err(|error| format!("failed to encode executables runtime: {error}"))
    }
}

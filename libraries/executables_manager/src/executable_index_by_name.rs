use std::collections::BTreeMap;

use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{executable_id::ExecutableId, executable_name::ExecutableName};

#[derive(Clone, Debug, Default, Decode, Deserialize, Encode, Serialize)]
pub struct ExecutableIndexByName {
    entries: BTreeMap<ExecutableName, ExecutableId>,
}

impl ExecutableIndexByName {
    pub fn get(&self, name: &ExecutableName) -> Option<ExecutableId> {
        self.entries.get(name).copied()
    }

    pub(crate) fn insert(&mut self, name: ExecutableName, id: ExecutableId) -> Result<(), String> {
        if let Some(existing) = self.entries.insert(name.clone(), id) {
            return Err(format!(
                "duplicate executable name {} for UUIDs {} and {}",
                name, existing, id
            ));
        }

        Ok(())
    }
}

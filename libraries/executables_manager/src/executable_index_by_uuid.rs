use std::collections::BTreeMap;

use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{executable_id::ExecutableId, executable_name::ExecutableName};

#[derive(Clone, Debug, Default, Decode, Deserialize, Encode, Serialize)]
pub struct ExecutableIndexByUuid {
    entries: BTreeMap<ExecutableId, ExecutableName>,
}

impl ExecutableIndexByUuid {
    pub fn get(&self, id: ExecutableId) -> Option<&ExecutableName> {
        self.entries.get(&id)
    }

    pub fn ids(&self) -> impl Iterator<Item = ExecutableId> + '_ {
        self.entries.keys().copied()
    }

    pub(crate) fn insert(&mut self, id: ExecutableId, name: ExecutableName) -> Result<(), String> {
        if let Some(existing) = self.entries.insert(id, name.clone()) {
            return Err(format!(
                "duplicate executable UUID {} for names {} and {}",
                id, existing, name
            ));
        }

        Ok(())
    }
}

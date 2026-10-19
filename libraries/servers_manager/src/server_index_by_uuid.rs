use std::collections::BTreeMap;

use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{ServerId, ServerIndexEntry};

#[derive(Clone, Debug, Default, Decode, Deserialize, Encode, Serialize)]
pub struct ServerIndexByUuid {
    entries: BTreeMap<ServerId, ServerIndexEntry>,
}

impl ServerIndexByUuid {
    pub fn get(&self, id: ServerId) -> Option<&ServerIndexEntry> {
        self.entries.get(&id)
    }

    pub fn entries(&self) -> impl Iterator<Item = (ServerId, &ServerIndexEntry)> {
        self.entries.iter().map(|(id, entry)| (*id, entry))
    }

    pub fn ids(&self) -> impl Iterator<Item = ServerId> + '_ {
        self.entries.keys().copied()
    }

    pub(crate) fn get_mut(&mut self, id: ServerId) -> Option<&mut ServerIndexEntry> {
        self.entries.get_mut(&id)
    }

    pub(crate) fn insert(&mut self, entry: ServerIndexEntry) -> Result<(), String> {
        let id = entry.id();

        if let Some(existing) = self.entries.insert(id, entry) {
            return Err(format!(
                "duplicate server UUID {} for names {} and {}",
                id,
                existing.name(),
                self.entries
                    .get(&id)
                    .map(ServerIndexEntry::name)
                    .ok_or_else(|| format!("server index entry disappeared: {id}"))?
            ));
        }

        Ok(())
    }
}

use std::collections::BTreeMap;

use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{ServerIndexEntry, ServerName};

#[derive(Clone, Debug, Default, Decode, Deserialize, Encode, Serialize)]
pub struct ServerIndexByName {
    entries: BTreeMap<ServerName, ServerIndexEntry>,
}

impl ServerIndexByName {
    pub fn get(&self, name: &ServerName) -> Option<&ServerIndexEntry> {
        self.entries.get(name)
    }

    pub fn entries(&self) -> impl Iterator<Item = (&ServerName, &ServerIndexEntry)> {
        self.entries.iter()
    }

    pub(crate) fn get_mut(&mut self, name: &ServerName) -> Option<&mut ServerIndexEntry> {
        self.entries.get_mut(name)
    }

    pub(crate) fn insert(&mut self, entry: ServerIndexEntry) -> Result<(), String> {
        let name = entry.name().clone();

        if let Some(existing) = self.entries.insert(name.clone(), entry) {
            return Err(format!(
                "duplicate server name {} for UUIDs {} and {}",
                name,
                existing.id(),
                self.entries
                    .get(&name)
                    .map(ServerIndexEntry::id)
                    .ok_or_else(|| format!("server index entry disappeared: {name}"))?
            ));
        }

        Ok(())
    }
}

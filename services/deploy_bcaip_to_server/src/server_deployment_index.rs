use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use bincode_next::{decode_from_slice, encode_to_vec};

use crate::server_deployment_index_entry::ServerDeploymentIndexEntry;

pub(crate) struct ServerDeploymentIndex {
    binary_path: PathBuf,
    entries: Vec<ServerDeploymentIndexEntry>,
}

impl ServerDeploymentIndex {
    pub(crate) fn load(
        ron_path: impl AsRef<Path>,
        binary_path: impl AsRef<Path>,
    ) -> Result<Self, String> {
        let ron_path = ron_path.as_ref();
        let binary_path = binary_path.as_ref().to_path_buf();

        let source = fs::read_to_string(ron_path)
            .map_err(|error| format!("failed to read {}: {error}", ron_path.display()))?;

        let mut entries: Vec<ServerDeploymentIndexEntry> = ron::from_str(&source)
            .map_err(|error| format!("failed to decode {}: {error}", ron_path.display()))?;

        for entry in &entries {
            entry.validate()?;
        }

        let mut keys = std::collections::HashSet::new();

        for entry in &entries {
            if !keys.insert(entry.server_key().to_string()) {
                return Err(format!("duplicate server key: {}", entry.server_key()));
            }
        }

        if let Ok(bytes) = fs::read(&binary_path) {
            let config = bincode_next::config::standard();

            if let Ok((previous, consumed)) =
                decode_from_slice::<Vec<ServerDeploymentIndexEntry>, _>(&bytes, config)
            {
                if consumed == bytes.len() {
                    let previous: HashMap<String, ServerDeploymentIndexEntry> = previous
                        .into_iter()
                        .map(|entry| (entry.server_key().to_string(), entry))
                        .collect();

                    for entry in &mut entries {
                        if let Some(previous) = previous.get(entry.server_key()) {
                            entry.take_runtime_status_from(previous);
                        }
                    }
                }
            }
        }

        let index = Self {
            binary_path,
            entries,
        };

        index.save()?;

        Ok(index)
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn entry(&self, index: usize) -> Option<&ServerDeploymentIndexEntry> {
        self.entries.get(index)
    }

    pub(crate) fn entry_mut(&mut self, index: usize) -> Option<&mut ServerDeploymentIndexEntry> {
        self.entries.get_mut(index)
    }

    pub(crate) fn save(&self) -> Result<(), String> {
        let config = bincode_next::config::standard();

        let encoded = encode_to_vec(&self.entries, config)
            .map_err(|error| format!("failed to encode server deployment index: {error}"))?;

        let temporary = self.binary_path.with_extension("bin.tmp");

        fs::write(&temporary, encoded)
            .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;

        fs::rename(&temporary, &self.binary_path).map_err(|error| {
            format!(
                "failed to replace {} from {}: {error}",
                self.binary_path.display(),
                temporary.display(),
            )
        })
    }
}

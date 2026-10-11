use crate::atomic_file::AtomicFile;
use crate::handover_phase::HandoverPhase;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const FILE_NAME: &str = "supervisor-handover.ron";

/// Durable state of the auto-update transaction in progress. Its presence,
/// never the backup file alone, is what tells a startup that a handover
/// was interrupted and in which phase.
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct HandoverJournal {
    #[serde(skip)]
    path: PathBuf,
    update_id: String,
    executable: String,
    phase: HandoverPhase,
}

impl HandoverJournal {
    pub(crate) fn begin(
        data_directory: &Path,
        update_id: &str,
        executable: &str,
    ) -> Result<Self, String> {
        let journal = Self {
            path: data_directory.join(FILE_NAME),
            update_id: update_id.to_string(),
            executable: executable.to_string(),
            phase: HandoverPhase::Prepared,
        };

        journal.persist()?;

        Ok(journal)
    }

    pub(crate) fn load(data_directory: &Path) -> Result<Option<Self>, String> {
        let path = data_directory.join(FILE_NAME);

        if !path.exists() {
            return Ok(None);
        }

        let source = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let mut journal: Self = ron::from_str(&source)
            .map_err(|error| format!("failed to decode {}: {error}", path.display()))?;

        journal.path = path;

        Ok(Some(journal))
    }

    pub(crate) fn discard(data_directory: &Path) -> Result<(), String> {
        let path = data_directory.join(FILE_NAME);

        if path.exists() {
            fs::remove_file(&path)
                .map_err(|error| format!("failed to remove {}: {error}", path.display()))?;
        }

        fs::File::open(data_directory)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| {
                format!(
                    "failed to sync directory {}: {error}",
                    data_directory.display()
                )
            })
    }

    pub(crate) fn advance(&mut self, phase: HandoverPhase) -> Result<(), String> {
        let previous = self.phase;

        self.phase = phase;

        self.persist().inspect_err(|_| self.phase = previous)
    }

    pub(crate) fn directory(&self) -> &Path {
        self.path.parent().unwrap_or_else(|| Path::new("."))
    }

    pub(crate) fn phase(&self) -> HandoverPhase {
        self.phase
    }

    pub(crate) fn update_id(&self) -> &str {
        &self.update_id
    }

    pub(crate) fn executable(&self) -> &str {
        &self.executable
    }

    pub(crate) fn authorizes_promotion(&self, executable: &str, update_id: Option<&str>) -> bool {
        self.phase == HandoverPhase::Releasing
            && self.executable == executable
            && update_id == Some(self.update_id.as_str())
    }

    fn persist(&self) -> Result<(), String> {
        let encoded = ron::ser::to_string(self)
            .map_err(|error| format!("failed to encode handover journal: {error}"))?;

        AtomicFile::write(&self.path, encoded.as_bytes())
    }
}

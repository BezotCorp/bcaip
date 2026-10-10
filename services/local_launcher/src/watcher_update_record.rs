use crate::watcher_update_phase::WatcherUpdatePhase;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Deserialize, Clone, Debug)]
pub(crate) struct WatcherUpdateRecord {
    update_id: String,
    product: String,
    phase: WatcherUpdatePhase,
}

impl WatcherUpdateRecord {
    pub(crate) fn committed_update_id(
        state_directory: &Path,
        product: &str,
    ) -> Result<Option<String>, String> {
        let path = state_directory.join(format!("{product}.ron"));

        if !path.exists() {
            return Ok(None);
        }

        let source = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

        let record: Self = ron::from_str(&source)
            .map_err(|error| format!("failed to decode {}: {error}", path.display()))?;

        if record.product != product {
            return Err(format!(
                "watcher state {} belongs to {}, expected {}",
                path.display(),
                record.product,
                product
            ));
        }

        match record.phase {
            WatcherUpdatePhase::Committed => Ok(Some(record.update_id)),
            WatcherUpdatePhase::Pending | WatcherUpdatePhase::Failed => Ok(None),
        }
    }
}

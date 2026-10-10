use crate::update_phase::UpdatePhase;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct UpdateRecord {
    pub(crate) update_id: String,
    pub(crate) product: String,
    pub(crate) phase: UpdatePhase,
    pub(crate) expected_size: u64,
    pub(crate) received_size: u64,
    pub(crate) sha256: String,
    pub(crate) error: Option<String>,
}

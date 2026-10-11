use crate::update_phase::UpdatePhase;
use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(
    Encode,
    Decode,
    Deserialize,
    Serialize,
    Clone,
    Debug,
)]
pub(crate) struct Update {
    pub(crate) update_id: String,
    pub(crate) phase: UpdatePhase,
    pub(crate) expected_size: u64,
    pub(crate) received_size: u64,
    pub(crate) sha256: String,
    pub(crate) error: Option<String>,
}

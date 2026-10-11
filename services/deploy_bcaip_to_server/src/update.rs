use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::update_phase::UpdatePhase;

#[derive(Clone, Debug, Decode, Deserialize, Encode, Serialize)]
pub(crate) struct Update {
    update_id: String,
    phase: UpdatePhase,
    expected_size: u64,
    received_size: u64,
    sha256: String,
    error: Option<String>,
}

use crate::update_phase::UpdatePhase;
use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Encode, Decode, Deserialize, Serialize, Clone, Debug)]
pub(crate) struct Update {
    update_id: String,
    phase: UpdatePhase,
    expected_size: u64,
    received_size: u64,
    sha256: String,
    error: Option<String>,
}

impl Update {
    pub(crate) fn new(update_id: String, expected_size: u64, sha256: String) -> Self {
        Self {
            update_id,
            phase: UpdatePhase::Pending,
            expected_size,
            received_size: 0,
            sha256,
            error: None,
        }
    }

    pub(crate) fn update_id(&self) -> &str {
        &self.update_id
    }

    pub(crate) fn expected_size(&self) -> u64 {
        self.expected_size
    }

    pub(crate) fn received_size(&self) -> u64 {
        self.received_size
    }

    pub(crate) fn sha256(&self) -> &str {
        &self.sha256
    }

    pub(crate) fn is_pending(&self) -> bool {
        self.phase == UpdatePhase::Pending
    }

    pub(crate) fn is_committed(&self) -> bool {
        self.phase == UpdatePhase::Committed
    }

    pub(crate) fn has_id(&self, update_id: &str) -> bool {
        self.update_id == update_id
    }

    pub(crate) fn has_metadata(&self, expected_size: u64, sha256: &str) -> bool {
        self.expected_size == expected_size && self.sha256.eq_ignore_ascii_case(sha256)
    }

    pub(crate) fn matches(&self, update_id: &str, expected_size: u64, sha256: &str) -> bool {
        self.has_id(update_id) && self.has_metadata(expected_size, sha256)
    }

    pub(crate) fn is_fully_received(&self) -> bool {
        self.received_size == self.expected_size
    }

    pub(crate) fn receive(&mut self, size: u64) -> Result<u64, String> {
        let next_size = self
            .received_size
            .checked_add(size)
            .ok_or_else(|| "received size overflow".to_string())?;

        if next_size > self.expected_size {
            return Err(format!(
                "received more bytes than expected: {} > {}",
                next_size, self.expected_size
            ));
        }

        self.received_size = next_size;

        Ok(next_size)
    }

    pub(crate) fn keep_pending_with_error(&mut self, error: String) {
        self.error = Some(error);
    }

    pub(crate) fn commit(&mut self) {
        self.phase = UpdatePhase::Committed;
        self.received_size = self.expected_size;
        self.error = None;
    }

    pub(crate) fn fail(&mut self, error: String) {
        self.phase = UpdatePhase::Failed;
        self.error = Some(error);
    }

    pub(crate) fn committed_update_id(&self) -> Option<&str> {
        self.is_committed().then_some(self.update_id())
    }
}

use bincode_next::{Decode, Encode};
use serde::Deserialize;
use std::num::NonZeroU16;

#[derive(Encode, Decode, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(transparent)]
pub(crate) struct ServerPort(NonZeroU16);

impl ServerPort {
    pub(crate) fn validate(&self) -> Result<(), String> {
        Ok(())
    }
}

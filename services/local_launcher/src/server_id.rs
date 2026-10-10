use crate::id::Id;
use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Encode, Decode, Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub(crate) struct ServerId(Id);

impl ServerId {
    pub(crate) fn validate(&self) -> Result<(), String> {
        self.0.validate()
    }
}

impl fmt::Display for ServerId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

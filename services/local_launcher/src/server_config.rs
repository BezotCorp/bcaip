use crate::server_id::ServerId;
use crate::server_location::ServerLocation;
use bincode_next::{Decode, Encode};
use serde::Deserialize;

#[derive(Encode, Decode, Deserialize, Clone, Debug)]
pub(crate) struct ServerConfig {
    pub(crate) id: ServerId,
    pub(crate) location: ServerLocation,
}

impl ServerConfig {
    pub(crate) fn validate(&self) -> Result<(), String> {
        self.id.validate()?;
        self.location.validate()
    }
}

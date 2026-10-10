use crate::remote_server_location::RemoteServerLocation;
use bincode_next::{Decode, Encode};
use crate::server_id::ServerId;
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Encode, Decode, Deserialize, Clone, Debug)]
pub(crate) struct RemoteServerConfig {
    pub(crate) id: ServerId,
    pub(crate) location: RemoteServerLocation,
}

impl RemoteServerConfig {
    pub(crate) fn validate_all(servers: &[Self]) -> Result<(), String> {
        let mut ids = HashSet::new();

        for server in servers {
            server.id.validate()?;
            server.location.validate()?;

            if !ids.insert(server.id.clone()) {
                return Err(format!("duplicate server: {}", server.id));
            }
        }

        Ok(())
    }
}

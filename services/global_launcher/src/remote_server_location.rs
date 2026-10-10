use crate::auth_ref::AuthRef;
use crate::server_host::ServerHost;
use crate::server_port::ServerPort;
use crate::server_protocol::ServerProtocol;
use bincode_next::{Decode, Encode};
use serde::Deserialize;
use std::path::{Component, Path};

#[derive(Encode, Decode, Deserialize, Clone, Debug)]
pub(crate) struct RemoteServerLocation {
    pub(crate) protocol: ServerProtocol,
    pub(crate) host: ServerHost,
    pub(crate) port: ServerPort,
    pub(crate) auth_ref: AuthRef,
    pub(crate) root_path: String,
}

impl RemoteServerLocation {
    pub(crate) fn validate(&self) -> Result<(), String> {
        self.protocol.validate()?;
        self.host.validate()?;
        self.port.validate()?;
        self.auth_ref.validate()?;
        validate_root_path(&self.root_path)
    }
}

fn validate_root_path(value: &str) -> Result<(), String> {
    let path = Path::new(value);

    if value.trim().is_empty() {
        return Err("remote server root_path must not be empty".to_string());
    }

    if !path.is_absolute() {
        return Err("remote server root_path must be absolute".to_string());
    }

    for component in path.components() {
        if matches!(component, Component::ParentDir | Component::CurDir) {
            return Err(
                "remote server root_path must not contain relative segments".to_string(),
            );
        }
    }

    Ok(())
}

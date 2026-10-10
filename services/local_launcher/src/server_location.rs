use crate::auth_ref::AuthRef;
use crate::server_host::ServerHost;
use crate::server_port::ServerPort;
use crate::server_protocol::ServerProtocol;
use bincode_next::{Decode, Encode};
use serde::Deserialize;
use std::path::{Component, Path, PathBuf};

#[derive(Encode, Decode, Deserialize, Clone, Debug)]
pub(crate) enum ServerLocation {
    Local {
        root_path: String,
    },
    Remote {
        protocol: ServerProtocol,
        host: ServerHost,
        port: ServerPort,
        auth_ref: AuthRef,
        root_path: String,
    },
}

impl ServerLocation {
    pub(crate) fn validate(&self) -> Result<(), String> {
        match self {
            Self::Local { root_path } => validate_root_path(root_path, "local"),
            Self::Remote {
                protocol: _,
                host,
                port,
                auth_ref,
                root_path,
            } => {
                host.validate()?;
                port.validate()?;
                auth_ref.validate()?;
                validate_root_path(root_path, "remote")
            }
        }
    }

    pub(crate) fn local_root_path(&self) -> Option<&Path> {
        match self {
            Self::Local { root_path } => Some(Path::new(root_path)),
            Self::Remote { .. } => None,
        }
    }

    pub(crate) fn local_executable_path(&self, relative_path: &Path) -> Option<PathBuf> {
        self.local_root_path().map(|root| root.join(relative_path))
    }

    pub(crate) fn is_remote(&self) -> bool {
        matches!(self, Self::Remote { .. })
    }
}

fn validate_root_path(value: &str, kind: &str) -> Result<(), String> {
    let path = Path::new(value);

    if value.trim().is_empty() {
        return Err(format!("{kind} server root_path must not be empty"));
    }

    if !path.is_absolute() {
        return Err(format!("{kind} server root_path must be absolute"));
    }

    for component in path.components() {
        if matches!(component, Component::ParentDir | Component::CurDir) {
            return Err(format!(
                "{kind} server root_path must not contain relative segments"
            ));
        }
    }

    Ok(())
}

use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{ServerId, ServerName, ServerPaths};

#[derive(Clone, Debug, Decode, Deserialize, Encode, Serialize)]
pub struct Server {
    id: ServerId,
    name: ServerName,
    is_deployment_target: bool,
    ssh_target: Option<String>,
    paths: ServerPaths,
}

impl Server {
    pub fn new(
        id: ServerId,
        name: ServerName,
        is_deployment_target: bool,
        ssh_target: Option<String>,
        paths: ServerPaths,
    ) -> Result<Self, String> {
        let server = Self {
            id,
            name,
            is_deployment_target,
            ssh_target,
            paths,
        };

        server.validate()?;

        Ok(server)
    }

    pub fn id(&self) -> ServerId {
        self.id
    }

    pub fn name(&self) -> &ServerName {
        &self.name
    }

    pub fn is_deployment_target(&self) -> bool {
        self.is_deployment_target
    }

    pub fn ssh_target(&self) -> Option<&str> {
        self.ssh_target.as_deref()
    }

    pub fn paths(&self) -> &ServerPaths {
        &self.paths
    }

    pub fn validate(&self) -> Result<(), String> {
        if let Some(ssh_target) = &self.ssh_target {
            if ssh_target.is_empty() {
                return Err("ssh_target must not be empty when present".to_string());
            }

            if ssh_target.bytes().any(|byte| byte.is_ascii_control()) {
                return Err("ssh_target contains control characters".to_string());
            }
        }

        self.paths.validate()
    }
}

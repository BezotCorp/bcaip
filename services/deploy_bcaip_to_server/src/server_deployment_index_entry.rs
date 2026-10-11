use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::server_deployment_status::ServerDeploymentStatus;

#[derive(Clone, Debug, Decode, Deserialize, Encode, Serialize)]
pub(crate) struct ServerDeploymentIndexEntry {
    server_key: String,
    deploy: bool,
    status: ServerDeploymentStatus,
}

impl ServerDeploymentIndexEntry {
    pub(crate) fn server_key(&self) -> &str {
        &self.server_key
    }

    pub(crate) fn should_deploy(&self) -> bool {
        self.deploy
    }

    pub(crate) fn status(&self) -> ServerDeploymentStatus {
        self.status
    }

    pub(crate) fn mark_pending(&mut self) {
        self.status = ServerDeploymentStatus::Pending;
    }

    pub(crate) fn mark_booted(&mut self) {
        self.status = ServerDeploymentStatus::Booted;
    }

    pub(crate) fn take_runtime_status_from(&mut self, previous: &Self) {
        self.status = previous.status;
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.server_key.is_empty()
            || !self
                .server_key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        {
            return Err(format!("invalid server key: {}", self.server_key));
        }

        Ok(())
    }
}

use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{ServerDeploymentStatus, ServerId, ServerName};

#[derive(Clone, Debug, Decode, Deserialize, Encode, Serialize)]
pub struct ServerIndexEntry {
    id: ServerId,
    name: ServerName,
    is_deployment_target: bool,

    #[serde(skip, default)]
    deployment_status: ServerDeploymentStatus,
}

impl ServerIndexEntry {
    pub(crate) fn new(
        id: ServerId,
        name: ServerName,
        is_deployment_target: bool,
        deployment_status: ServerDeploymentStatus,
    ) -> Self {
        Self {
            id,
            name,
            is_deployment_target,
            deployment_status,
        }
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

    pub fn deployment_status(&self) -> ServerDeploymentStatus {
        self.deployment_status
    }

    pub(crate) fn set_deployment_status(&mut self, deployment_status: ServerDeploymentStatus) {
        self.deployment_status = deployment_status;
    }
}

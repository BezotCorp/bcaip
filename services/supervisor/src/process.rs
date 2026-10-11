use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Encode, Decode, Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub(crate) struct Process {
    pid: u32,
    instance_id: String,
    ws_address: SocketAddr,
    applied_update_id: Option<String>,
}

impl Process {
    pub(crate) fn new(
        pid: u32,
        instance_id: String,
        ws_address: SocketAddr,
        applied_update_id: Option<String>,
    ) -> Self {
        Self {
            pid,
            instance_id,
            ws_address,
            applied_update_id,
        }
    }

    pub(crate) fn instance_id(&self) -> &str {
        &self.instance_id
    }

    pub(crate) fn instance_id_owned(&self) -> String {
        self.instance_id.clone()
    }

    pub(crate) fn ws_address(&self) -> SocketAddr {
        self.ws_address
    }

    pub(crate) fn applied_update_id(&self) -> Option<&str> {
        self.applied_update_id.as_deref()
    }

    pub(crate) fn has_instance_id(&self, instance_id: &str) -> bool {
        self.instance_id == instance_id
    }

    pub(crate) fn has_applied_update_id(&self, update_id: &str) -> bool {
        self.applied_update_id.as_deref() == Some(update_id)
    }
}

use std::net::SocketAddr;

use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{process::Process, update::Update};

#[derive(Clone, Debug, Decode, Deserialize, Encode, Serialize)]
pub(crate) struct Executable {
    name: String,
    working_directory: String,
    executable_path: String,
    default_ws_address: SocketAddr,

    #[serde(default)]
    process: Option<Process>,

    #[serde(default)]
    update: Option<Update>,
}

impl Executable {
    pub(crate) fn is_named(&self, name: &str) -> bool {
        self.name == name
    }

    pub(crate) fn default_ws_address(&self) -> SocketAddr {
        self.default_ws_address
    }
}

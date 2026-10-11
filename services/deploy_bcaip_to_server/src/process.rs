use std::net::SocketAddr;

use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Decode, Deserialize, Encode, Eq, PartialEq, Serialize)]
pub(crate) struct Process {
    pid: u32,
    instance_id: String,
    ws_address: SocketAddr,
    applied_update_id: Option<String>,
}

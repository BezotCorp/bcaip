use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(
    Encode,
    Decode,
    Deserialize,
    Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
)]
pub(crate) struct Process {
    pub(crate) pid: u32,
    pub(crate) instance_id: String,
    pub(crate) ws_address: SocketAddr,
    pub(crate) applied_update_id: Option<String>,
}

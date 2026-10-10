use bincode_next::{Decode, Encode};
use serde::Deserialize;
use std::net::SocketAddr;

#[derive(Encode, Decode, Deserialize, Clone, Debug)]
pub(crate) enum HealthCheck {
    WebSocketPing { address: SocketAddr },
}

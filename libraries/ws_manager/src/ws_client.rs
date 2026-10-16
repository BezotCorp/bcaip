use std::{
    net::{SocketAddr, TcpStream},
    time::Duration,
};

use tungstenite::client;

use crate::WsConnection;

pub struct WsClient {
    address: SocketAddr,
}

impl WsClient {
    pub fn new(address: SocketAddr) -> Self {
        Self { address }
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn connect(
        &self,
        connect_timeout: Duration,
        read_timeout: Option<Duration>,
        write_timeout: Option<Duration>,
    ) -> Result<WsConnection, String> {
        let stream = TcpStream::connect_timeout(&self.address, connect_timeout)
            .map_err(|error| format!("failed to connect to {}: {error}", self.address))?;

        stream
            .set_read_timeout(read_timeout)
            .map_err(|error| format!("failed to set WebSocket read timeout: {error}"))?;

        stream
            .set_write_timeout(write_timeout)
            .map_err(|error| format!("failed to set WebSocket write timeout: {error}"))?;

        let (socket, _) = client(format!("ws://{}/", self.address), stream)
            .map_err(|error| format!("WebSocket client handshake failed: {error}"))?;

        Ok(WsConnection::new(socket))
    }
}

use std::{net::TcpStream, time::Duration};

use tungstenite::accept;

use crate::WsConnection;

#[derive(Clone, Copy)]
pub struct WsServer {
    read_timeout: Option<Duration>,
    write_timeout: Option<Duration>,
}

impl WsServer {
    pub fn new(read_timeout: Option<Duration>, write_timeout: Option<Duration>) -> Self {
        Self {
            read_timeout,
            write_timeout,
        }
    }

    pub fn accept(&self, stream: TcpStream) -> Result<WsConnection, String> {
        stream
            .set_read_timeout(self.read_timeout)
            .map_err(|error| format!("failed to set WebSocket read timeout: {error}"))?;

        stream
            .set_write_timeout(self.write_timeout)
            .map_err(|error| format!("failed to set WebSocket write timeout: {error}"))?;

        let socket = accept(stream)
            .map_err(|error| format!("WebSocket server handshake failed: {error}"))?;

        Ok(WsConnection::new(socket))
    }
}

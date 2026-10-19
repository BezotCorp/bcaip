use std::{net::TcpStream, time::Duration};

use tungstenite::{Message, WebSocket};

use crate::WsMessage;

pub struct WsConnection {
    socket: WebSocket<TcpStream>,
}

impl WsConnection {
    pub(crate) fn new(socket: WebSocket<TcpStream>) -> Self {
        Self { socket }
    }

    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> Result<(), String> {
        self.socket
            .get_ref()
            .set_read_timeout(timeout)
            .map_err(|error| format!("failed to set WebSocket read timeout: {error}"))
    }

    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> Result<(), String> {
        self.socket
            .get_ref()
            .set_write_timeout(timeout)
            .map_err(|error| format!("failed to set WebSocket write timeout: {error}"))
    }

    pub fn read(&mut self) -> Result<WsMessage, String> {
        let message = self
            .socket
            .read()
            .map_err(|error| format!("failed to read WebSocket message: {error}"))?;

        Ok(match message {
            Message::Text(value) => WsMessage::Text(value.to_string()),
            Message::Binary(value) => WsMessage::Binary(value.to_vec()),
            Message::Ping(value) => WsMessage::Ping(value.to_vec()),
            Message::Pong(value) => WsMessage::Pong(value.to_vec()),
            Message::Close(_) => WsMessage::Close,
            Message::Frame(_) => WsMessage::Frame,
        })
    }

    pub fn send_text(&mut self, value: &str) -> Result<(), String> {
        self.socket
            .send(Message::Text(value.to_string().into()))
            .map_err(|error| format!("failed to send WebSocket text: {error}"))
    }

    pub fn send_binary(&mut self, value: &[u8]) -> Result<(), String> {
        self.socket
            .send(Message::Binary(value.to_vec().into()))
            .map_err(|error| format!("failed to send WebSocket binary data: {error}"))
    }

    pub fn send_ping(&mut self, payload: &[u8]) -> Result<(), String> {
        self.socket
            .send(Message::Ping(payload.to_vec().into()))
            .map_err(|error| format!("failed to send WebSocket Ping: {error}"))
    }

    pub fn send_pong(&mut self, payload: &[u8]) -> Result<(), String> {
        self.socket
            .send(Message::Pong(payload.to_vec().into()))
            .map_err(|error| format!("failed to send WebSocket Pong: {error}"))
    }

    pub fn flush(&mut self) -> Result<(), String> {
        self.socket
            .flush()
            .map_err(|error| format!("failed to flush WebSocket connection: {error}"))
    }

    pub fn close(&mut self) -> Result<(), String> {
        self.socket
            .close(None)
            .map_err(|error| format!("failed to close WebSocket connection: {error}"))
    }
}

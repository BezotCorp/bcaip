use crate::watcher_request::WatcherRequest;
use crate::watcher_response::WatcherResponse;
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;
use tungstenite::{Message, client};

const TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) fn send(
    address: SocketAddr,
    request: &WatcherRequest,
) -> Result<WatcherResponse, String> {
    let stream = TcpStream::connect_timeout(&address, TIMEOUT)
        .map_err(|error| format!("failed to connect to watcher {}: {}", address, error))?;

    stream
        .set_read_timeout(Some(TIMEOUT))
        .map_err(|error| format!("failed to set watcher read timeout: {}", error))?;

    stream
        .set_write_timeout(Some(TIMEOUT))
        .map_err(|error| format!("failed to set watcher write timeout: {}", error))?;

    let (mut socket, _) = client(format!("ws://{address}/"), stream)
        .map_err(|error| format!("watcher WebSocket connection failed: {}", error))?;

    let serialized = ron::ser::to_string(request)
        .map_err(|error| format!("failed to encode watcher request: {}", error))?;

    socket
        .send(Message::Text(serialized.into()))
        .map_err(|error| format!("failed to send watcher request: {}", error))?;

    let message = socket
        .read()
        .map_err(|error| format!("failed to read watcher response: {}", error))?;

    let Message::Text(text) = message else {
        return Err("watcher response was not text".to_string());
    };

    let response: WatcherResponse = ron::from_str(text.as_str())
        .map_err(|error| format!("failed to decode watcher response: {}", error))?;

    match response {
        WatcherResponse::Failure { message } => Err(message),

        response => Ok(response),
    }
}

pub(crate) fn acknowledge(address: SocketAddr, request: &WatcherRequest) -> Result<(), String> {
    match send(address, request)? {
        WatcherResponse::Acknowledged => Ok(()),

        response => Err(format!("unexpected watcher response: {:?}", response)),
    }
}

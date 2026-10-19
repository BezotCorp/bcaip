use std::error::Error;
use std::net::{TcpListener, TcpStream};

use storage_exchange_format::{StorageRequest, decode, encode};
use tungstenite::{Message, accept};

use crate::request_handler::handle_request;
use crate::storage_engine::StorageEngine;
use crate::storage_service::StorageService;

pub fn run_server<E: StorageEngine>(
    address: &str,
    storage: &mut StorageService<E>,
) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(address)?;

    eprintln!("Storage listening on {}", listener.local_addr()?);

    for incoming in listener.incoming() {
        match incoming {
            Ok(stream) => {
                if let Err(error) = serve_connection(stream, storage) {
                    eprintln!("Storage connection failed: {error}");
                }
            }
            Err(error) => eprintln!("Storage accept failed: {error}"),
        }
    }

    Ok(())
}

fn serve_connection<E: StorageEngine>(
    stream: TcpStream,
    storage: &mut StorageService<E>,
) -> Result<(), Box<dyn Error>> {
    let mut socket = accept(stream)?;

    loop {
        let message = match socket.read() {
            Ok(message) => message,
            Err(tungstenite::Error::ConnectionClosed) => break,
            Err(tungstenite::Error::AlreadyClosed) => break,
            Err(error) => return Err(error.into()),
        };

        match message {
            Message::Binary(bytes) => {
                let request: StorageRequest = match decode(&bytes) {
                    Ok(request) => request,
                    Err(error) => {
                        eprintln!("Invalid Storage request: {error}");
                        break;
                    }
                };

                let response = handle_request(storage, request);
                let encoded = encode(&response)?;

                socket.send(Message::Binary(encoded.into()))?;
            }
            Message::Close(_) => break,
            Message::Ping(bytes) => {
                socket.send(Message::Pong(bytes))?;
            }
            Message::Pong(_) => {}
            Message::Text(_) => break,
            _ => {}
        }
    }

    Ok(())
}

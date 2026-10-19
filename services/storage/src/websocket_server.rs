use std::error::Error;
use std::io;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use storage_exchange_format::{StorageRequest, decode, encode};
use tungstenite::{Message, accept};

use crate::request_handler::handle_request;
use crate::storage_engine::StorageEngine;
use crate::storage_service::StorageService;

const MAX_CONNECTIONS: usize = 64;

struct ConnectionGuard {
    active: Arc<AtomicUsize>,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::AcqRel);
    }
}

pub fn run_server<E: StorageEngine + Send + 'static>(
    address: &str,
    storage: StorageService<E>,
) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(address)?;
    let storage = Arc::new(Mutex::new(storage));
    let active = Arc::new(AtomicUsize::new(0));

    println!("Storage listening on {}", listener.local_addr()?);

    for incoming in listener.incoming() {
        let stream = match incoming {
            Ok(stream) => stream,
            Err(error) => {
                println!("Storage accept failed: {error}");
                continue;
            }
        };

        if active
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < MAX_CONNECTIONS).then_some(count + 1)
            })
            .is_err()
        {
            drop(stream);
            continue;
        }

        let storage = Arc::clone(&storage);
        let guard = ConnectionGuard {
            active: Arc::clone(&active),
        };

        thread::spawn(move || {
            let _guard = guard;

            if let Err(error) = serve_connection(stream, &storage) {
                println!("Storage connection failed: {error}");
            }
        });
    }

    Ok(())
}

fn serve_connection<E: StorageEngine>(
    stream: TcpStream,
    storage: &Mutex<StorageService<E>>,
) -> Result<(), Box<dyn Error>> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;

    let mut socket = accept(stream)?;

    socket.get_mut().set_read_timeout(None)?;

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
                        println!("Invalid Storage request: {error}");
                        break;
                    }
                };

                let response = {
                    let mut storage = storage
                        .lock()
                        .map_err(|_| io::Error::other("Storage lock poisoned"))?;

                    handle_request(&mut *storage, request)
                };

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

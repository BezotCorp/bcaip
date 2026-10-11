use crate::executable::Executable;
use crate::watcher_request::WatcherRequest;
use crate::watcher_response::WatcherResponse;
use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use tungstenite::{Message, accept, client};

const TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) fn is_healthy(executable: &Executable) -> bool {
    let Some(process) = executable.process.as_ref() else {
        return false;
    };

    let Ok(stream) = TcpStream::connect_timeout(&process.ws_address, TIMEOUT) else {
        return false;
    };

    if stream.set_read_timeout(Some(TIMEOUT)).is_err()
        || stream.set_write_timeout(Some(TIMEOUT)).is_err()
    {
        return false;
    }

    let Ok((mut socket, _)) = client(format!("ws://{}/", process.ws_address), stream) else {
        return false;
    };

    let Ok(request) = ron::ser::to_string(&WatcherRequest::Health) else {
        return false;
    };

    if socket.send(Message::Text(request.into())).is_err() {
        return false;
    }

    loop {
        match socket.read() {
            Ok(Message::Text(text)) => {
                let Ok(response) = ron::from_str::<WatcherResponse>(text.as_str()) else {
                    return false;
                };

                return matches!(
                    response,
                    WatcherResponse::Healthy {
                        executable: name,
                        instance_id,
                        applied_update_id,
                    } if name == executable.name
                        && instance_id
                            == process.instance_id
                        && applied_update_id
                            .as_deref()
                            == process
                                .applied_update_id
                                .as_deref()
                );
            }

            Ok(Message::Ping(payload)) => {
                if socket.send(Message::Pong(payload)).is_err() {
                    return false;
                }
            }

            Ok(Message::Pong(_)) => {}

            Ok(_) | Err(_) => {
                return false;
            }
        }
    }
}

pub(crate) fn bind(address: SocketAddr) -> io::Result<TcpListener> {
    let listener = TcpListener::bind(address)?;

    listener.set_nonblocking(true)?;

    Ok(listener)
}

pub(crate) fn serve(
    listener: TcpListener,
    stop: &AtomicBool,
    executable: String,
    instance_id: String,
    applied_update_id: Option<String>,
) {
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                let executable = executable.clone();

                let instance_id = instance_id.clone();

                let applied_update_id = applied_update_id.clone();

                thread::spawn(move || {
                    handle(stream, executable, instance_id, applied_update_id);
                });
            }

            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }

            Err(error) => {
                println!("[WS] accept failed: {}", error);

                thread::sleep(Duration::from_millis(250));
            }
        }
    }
}

fn handle(
    stream: TcpStream,
    executable: String,
    instance_id: String,
    applied_update_id: Option<String>,
) {
    let _ = stream.set_read_timeout(Some(TIMEOUT));

    let _ = stream.set_write_timeout(Some(TIMEOUT));

    let Ok(mut socket) = accept(stream) else {
        return;
    };

    loop {
        match socket.read() {
            Ok(Message::Text(text)) => {
                let Ok(request) = ron::from_str::<WatcherRequest>(text.as_str()) else {
                    break;
                };

                let response = match request {
                    WatcherRequest::Health => WatcherResponse::Healthy {
                        executable: executable.clone(),
                        instance_id: instance_id.clone(),
                        applied_update_id: applied_update_id.clone(),
                    },

                    _ => WatcherResponse::Failure {
                        message: "unsupported launcher request".to_string(),
                    },
                };

                let Ok(serialized) = ron::ser::to_string(&response) else {
                    break;
                };

                if socket.send(Message::Text(serialized.into())).is_err() {
                    break;
                }
            }

            Ok(Message::Ping(payload)) => {
                if socket.send(Message::Pong(payload)).is_err() {
                    break;
                }
            }

            Ok(Message::Close(_)) => {
                break;
            }

            Ok(_) => {}

            Err(_) => {
                break;
            }
        }
    }
}

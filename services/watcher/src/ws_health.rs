use crate::executable::Executable;
use crate::watcher_request::WatcherRequest;
use crate::watcher_response::WatcherResponse;
use std::net::TcpStream;
use std::time::Duration;
use tungstenite::{client, Message};

const TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) fn is_healthy(
    executable: &Executable,
) -> bool {
    let Some(process) =
        executable.process.as_ref()
    else {
        return false;
    };

    let Ok(stream) =
        TcpStream::connect_timeout(
            &process.ws_address,
            TIMEOUT,
        )
    else {
        return false;
    };

    if stream
        .set_read_timeout(
            Some(TIMEOUT),
        )
        .is_err()
        || stream
            .set_write_timeout(
                Some(TIMEOUT),
            )
            .is_err()
    {
        return false;
    }

    let Ok((mut socket, _)) =
        client(
            format!(
                "ws://{}/",
                process.ws_address
            ),
            stream,
        )
    else {
        return false;
    };

    let Ok(request) =
        ron::ser::to_string(
            &WatcherRequest::Health,
        )
    else {
        return false;
    };

    if socket
        .send(
            Message::Text(
                request.into(),
            ),
        )
        .is_err()
    {
        return false;
    }

    loop {
        match socket.read() {
            Ok(Message::Text(text)) => {
                let Ok(response) =
                    ron::from_str::<
                        WatcherResponse
                    >(
                        text.as_str(),
                    )
                else {
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
                if socket
                    .send(
                        Message::Pong(
                            payload,
                        ),
                    )
                    .is_err()
                {
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

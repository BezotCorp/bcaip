use executables_manager::Executable;
use executables_manager::{ExecutableManagementRequest, ExecutableManagementResponse};
use std::time::Duration;
use ws_manager::{WsClient, WsMessage};

const TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) struct WsHealth;

impl WsHealth {
    pub(crate) fn is_healthy(executable: &Executable) -> bool {
        let Some(process) = executable.process() else {
            return false;
        };

        let client = WsClient::new(process.ws_address());

        let Ok(mut connection) = client.connect(TIMEOUT, Some(TIMEOUT), Some(TIMEOUT)) else {
            return false;
        };

        let Ok(request) = ron::ser::to_string(&ExecutableManagementRequest::Health) else {
            return false;
        };

        if connection.send_text(&request).is_err() {
            return false;
        }

        loop {
            match connection.read() {
                Ok(WsMessage::Text(text)) => {
                    let Ok(response) = ron::from_str::<ExecutableManagementResponse>(&text) else {
                        return false;
                    };

                    return matches!(
                        response,
                        ExecutableManagementResponse::Healthy {
                            executable: name,
                            instance_id,
                            applied_update_id,
                        } if executable.is_named(&name)
                            && process.has_instance_id(&instance_id)
                            && applied_update_id.as_deref()
                                == process.applied_update_id().as_deref()
                    );
                }

                Ok(WsMessage::Ping(payload)) => {
                    if connection.send_pong(&payload).is_err() {
                        return false;
                    }
                }

                Ok(WsMessage::Pong(_)) => {}

                Ok(_) | Err(_) => return false,
            }
        }
    }
}

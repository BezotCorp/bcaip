use std::{
    net::{SocketAddr, TcpListener},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use executables_manager::{ExecutableManagementRequest, ExecutableManagementResponse};
use servers_manager::Server;
use ws_manager::{WsClient, WsMessage};

const TIMEOUT: Duration = Duration::from_secs(2);
const TUNNEL_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) struct SupervisorHealth;

impl SupervisorHealth {
    pub(crate) fn check(
        server: &Server,
        supervisor_address: SocketAddr,
        expected_executable: &str,
    ) -> Result<bool, String> {
        let ssh_target = server.ssh_target().ok_or_else(|| {
            format!(
                "server {} has no ssh_target; remote supervisor Health cannot be checked",
                server.name()
            )
        })?;

        let local_port = Self::available_local_port()?;

        let mut tunnel = Self::open_tunnel(ssh_target, supervisor_address, local_port)?;

        let result = Self::check_local_port(local_port, expected_executable);

        let _ = tunnel.kill();
        let _ = tunnel.wait();

        result
    }

    fn available_local_port() -> Result<u16, String> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|error| format!("failed to reserve local port: {error}"))?;

        listener
            .local_addr()
            .map(|address| address.port())
            .map_err(|error| format!("failed to read local port: {error}"))
    }

    fn open_tunnel(
        ssh_target: &str,
        supervisor_address: SocketAddr,
        local_port: u16,
    ) -> Result<Child, String> {
        let forwarding = format!(
            "127.0.0.1:{local_port}:{}:{}",
            supervisor_address.ip(),
            supervisor_address.port(),
        );

        Command::new("ssh")
            .arg("-o")
            .arg("BatchMode=yes")
            .arg("-o")
            .arg("ExitOnForwardFailure=yes")
            .arg("-N")
            .arg("-L")
            .arg(forwarding)
            .arg(ssh_target)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("failed to open SSH tunnel: {error}"))
    }

    fn check_local_port(local_port: u16, expected_executable: &str) -> Result<bool, String> {
        let address: SocketAddr = format!("127.0.0.1:{local_port}")
            .parse()
            .map_err(|error| format!("invalid local supervisor address: {error}"))?;

        let client = WsClient::new(address);
        let deadline = Instant::now() + TUNNEL_TIMEOUT;

        let mut connection = loop {
            match client.connect(TIMEOUT, Some(TIMEOUT), Some(TIMEOUT)) {
                Ok(connection) => break connection,

                Err(_) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(100));
                }

                Err(_) => return Ok(false),
            }
        };

        let request = ron::ser::to_string(&ExecutableManagementRequest::Health)
            .map_err(|error| format!("failed to encode Health request: {error}"))?;

        connection
            .send_text(&request)
            .map_err(|error| format!("failed to send Health request: {error}"))?;

        loop {
            match connection.read() {
                Ok(WsMessage::Text(text)) => {
                    let response: ExecutableManagementResponse = ron::from_str(&text)
                        .map_err(|error| format!("invalid supervisor Health response: {error}"))?;

                    return Ok(matches!(
                        response,
                        ExecutableManagementResponse::Healthy {
                            executable,
                            instance_id,
                            ..
                        } if executable == expected_executable && !instance_id.is_empty()
                    ));
                }

                Ok(WsMessage::Ping(payload)) => {
                    connection
                        .send_pong(&payload)
                        .map_err(|error| format!("failed to send Pong: {error}"))?;
                }

                Ok(WsMessage::Pong(_)) => {}

                Ok(_) | Err(_) => return Ok(false),
            }
        }
    }
}

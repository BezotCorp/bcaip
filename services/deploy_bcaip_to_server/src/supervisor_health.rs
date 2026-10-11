use std::{
    net::{SocketAddr, TcpListener, TcpStream},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use tungstenite::{Message, client};

use crate::{
    server_deployment_descriptor::ServerDeploymentDescriptor,
    supervisor_health_request::SupervisorHealthRequest,
    supervisor_health_response::SupervisorHealthResponse,
};

const TIMEOUT: Duration = Duration::from_secs(2);
const TUNNEL_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) struct SupervisorHealth;

impl SupervisorHealth {
    pub(crate) fn check(
        descriptor: &ServerDeploymentDescriptor,
        supervisor_address: SocketAddr,
    ) -> Result<bool, String> {
        let local_port = Self::available_local_port()?;
        let mut tunnel = Self::open_tunnel(descriptor, supervisor_address, local_port)?;

        let result = Self::check_local_port(local_port);

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
        descriptor: &ServerDeploymentDescriptor,
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
            .arg(descriptor.ssh_target())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("failed to open SSH tunnel: {error}"))
    }

    fn check_local_port(local_port: u16) -> Result<bool, String> {
        let address: SocketAddr = format!("127.0.0.1:{local_port}")
            .parse()
            .map_err(|error| format!("invalid local supervisor address: {error}"))?;

        let deadline = Instant::now() + TUNNEL_TIMEOUT;

        let stream = loop {
            match TcpStream::connect_timeout(&address, TIMEOUT) {
                Ok(stream) => break stream,
                Err(_) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(100));
                }
                Err(_) => return Ok(false),
            }
        };

        stream
            .set_read_timeout(Some(TIMEOUT))
            .map_err(|error| format!("failed to set read timeout: {error}"))?;

        stream
            .set_write_timeout(Some(TIMEOUT))
            .map_err(|error| format!("failed to set write timeout: {error}"))?;

        let (mut socket, _) = client(format!("ws://127.0.0.1:{local_port}/"), stream)
            .map_err(|error| format!("WebSocket connection failed: {error}"))?;

        let request = ron::ser::to_string(&SupervisorHealthRequest::Health)
            .map_err(|error| format!("failed to encode Health request: {error}"))?;

        socket
            .send(Message::Text(request.into()))
            .map_err(|error| format!("failed to send Health request: {error}"))?;

        loop {
            match socket.read() {
                Ok(Message::Text(text)) => {
                    let response: SupervisorHealthResponse = ron::from_str(text.as_str())
                        .map_err(|error| format!("invalid supervisor Health response: {error}"))?;

                    return Ok(response.is_healthy_supervisor());
                }

                Ok(Message::Ping(payload)) => {
                    socket
                        .send(Message::Pong(payload))
                        .map_err(|error| format!("failed to send Pong: {error}"))?;
                }

                Ok(Message::Pong(_)) => {}

                Ok(_) | Err(_) => return Ok(false),
            }
        }
    }
}

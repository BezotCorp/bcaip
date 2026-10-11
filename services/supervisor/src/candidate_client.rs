use crate::candidate_report::CandidateReport;
use crate::executable_management_request::ExecutableManagementRequest;
use crate::executable_management_response::ExecutableManagementResponse;
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;
use tungstenite::{Message, client};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const REPORT_TIMEOUT: Duration = Duration::from_secs(2);
const PROMOTION_TIMEOUT: Duration = Duration::from_secs(15);

pub(crate) struct CandidateClient {
    address: SocketAddr,
    executable: String,
    instance_id: String,
}

impl CandidateClient {
    pub(crate) fn new(address: SocketAddr, executable: &str, instance_id: &str) -> Self {
        Self {
            address,
            executable: executable.to_string(),
            instance_id: instance_id.to_string(),
        }
    }

    pub(crate) fn address(&self) -> SocketAddr {
        self.address
    }

    pub(crate) fn instance_id(&self) -> &str {
        &self.instance_id
    }

    pub(crate) fn report(&self) -> Result<CandidateReport, String> {
        match self.exchange(ExecutableManagementRequest::CandidateReport, REPORT_TIMEOUT)? {
            ExecutableManagementResponse::Candidate(report) => Ok(report),
            ExecutableManagementResponse::Failure { message } => Err(message),
            response => Err(format!("unexpected candidate response: {response:?}")),
        }
    }

    pub(crate) fn promote(&self) -> Result<(), String> {
        let request = ExecutableManagementRequest::PromoteCandidate {
            executable: self.executable.clone(),
            instance_id: self.instance_id.clone(),
        };

        match self.exchange(request, PROMOTION_TIMEOUT)? {
            ExecutableManagementResponse::Promoted => Ok(()),
            ExecutableManagementResponse::Failure { message } => Err(message),
            response => Err(format!("unexpected promotion response: {response:?}")),
        }
    }

    fn exchange(
        &self,
        request: ExecutableManagementRequest,
        timeout: Duration,
    ) -> Result<ExecutableManagementResponse, String> {
        let stream = TcpStream::connect_timeout(&self.address, CONNECT_TIMEOUT)
            .map_err(|error| format!("failed to connect to candidate: {error}"))?;
        stream
            .set_read_timeout(Some(timeout))
            .map_err(|error| format!("failed to set candidate read timeout: {error}"))?;
        stream
            .set_write_timeout(Some(CONNECT_TIMEOUT))
            .map_err(|error| format!("failed to set candidate write timeout: {error}"))?;

        let (mut socket, _) = client(format!("ws://{}/", self.address), stream)
            .map_err(|error| format!("candidate WebSocket handshake failed: {error}"))?;
        let request = ron::ser::to_string(&request)
            .map_err(|error| format!("failed to encode candidate request: {error}"))?;

        socket
            .send(Message::Text(request.into()))
            .map_err(|error| format!("failed to send candidate request: {error}"))?;

        loop {
            match socket
                .read()
                .map_err(|error| format!("failed to read candidate response: {error}"))?
            {
                Message::Text(text) => {
                    return ron::from_str(text.as_str())
                        .map_err(|error| format!("invalid candidate response: {error}"));
                }

                Message::Ping(payload) => {
                    socket
                        .send(Message::Pong(payload))
                        .map_err(|error| format!("failed to answer candidate Ping: {error}"))?;
                }

                _ => return Err("candidate response was not text".to_string()),
            }
        }
    }
}

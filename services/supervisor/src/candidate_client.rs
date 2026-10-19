use executables_manager::{
    CandidateReport, ExecutableManagementRequest, ExecutableManagementResponse,
};
use std::net::SocketAddr;
use std::time::Duration;
use ws_manager::{WsClient, WsMessage};

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
        let client = WsClient::new(self.address);

        let mut connection = client
            .connect(CONNECT_TIMEOUT, Some(timeout), Some(CONNECT_TIMEOUT))
            .map_err(|error| format!("failed to connect to candidate: {error}"))?;

        let request = ron::ser::to_string(&request)
            .map_err(|error| format!("failed to encode candidate request: {error}"))?;

        connection
            .send_text(&request)
            .map_err(|error| format!("failed to send candidate request: {error}"))?;

        loop {
            match connection
                .read()
                .map_err(|error| format!("failed to read candidate response: {error}"))?
            {
                WsMessage::Text(text) => {
                    return ron::from_str(&text)
                        .map_err(|error| format!("invalid candidate response: {error}"));
                }

                WsMessage::Ping(payload) => {
                    connection
                        .send_pong(&payload)
                        .map_err(|error| format!("failed to answer candidate Ping: {error}"))?;
                }

                _ => return Err("candidate response was not text".to_string()),
            }
        }
    }
}

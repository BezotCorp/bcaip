use crate::executable::Executable;
use std::net::SocketAddr;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const EXIT_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// The candidate is a child of the authoritative supervisor, so it lives in
/// the persistent unit's cgroup: systemd kills it with the unit if the
/// supervisor dies, and can adopt it as main process once it is promoted.
pub(crate) struct CandidateProcess {
    child: Child,
}

impl CandidateProcess {
    pub(crate) fn spawn(
        executable: &Executable,
        instance_id: &str,
        candidate_address: SocketAddr,
        supervisor_address: SocketAddr,
    ) -> Result<Self, String> {
        let mut command = Command::new(executable.path());

        command
            .current_dir(executable.working_directory())
            .env("BCAIP_EXECUTABLE_NAME", executable.name())
            .env("BCAIP_SUPERVISOR_CANDIDATE", "1")
            .env(
                "BCAIP_SUPERVISOR_BIND_ADDRESS",
                candidate_address.to_string(),
            )
            .env("BCAIP_INSTANCE_ID", instance_id)
            .env("BCAIP_WS_ADDRESS", candidate_address.to_string())
            .env("BCAIP_SUPERVISOR_ADDRESS", supervisor_address.to_string())
            .stdin(Stdio::null());

        if let Some(update_id) = executable.committed_update_id() {
            command.env("BCAIP_APPLIED_UPDATE_ID", update_id);
        }

        let child = command.spawn().map_err(|error| {
            format!(
                "failed to spawn supervisor candidate {}: {error}",
                executable.path().display()
            )
        })?;

        Ok(Self { child })
    }

    pub(crate) fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Kills the candidate and waits for it, for at most `timeout`.
    pub(crate) fn terminate(mut self, timeout: Duration) -> Result<(), String> {
        if matches!(self.child.try_wait(), Ok(Some(_))) {
            return Ok(());
        }

        self.child
            .kill()
            .map_err(|error| format!("failed to kill candidate {}: {error}", self.child.id()))?;

        let deadline = Instant::now() + timeout;

        while Instant::now() < deadline {
            match self.child.try_wait() {
                Ok(Some(_)) => return Ok(()),
                Ok(None) => thread::sleep(EXIT_POLL_INTERVAL),
                Err(error) => {
                    return Err(format!(
                        "failed to wait for candidate {}: {error}",
                        self.child.id()
                    ));
                }
            }
        }

        Err(format!(
            "candidate {} did not exit within {timeout:?} after being killed",
            self.child.id()
        ))
    }
}

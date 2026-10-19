use crate::managed_process::ManagedProcess;
use executables_manager::Executable;
use std::net::SocketAddr;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

/// The candidate is a child of the authoritative supervisor, so it lives in
/// the persistent unit's cgroup: systemd kills it with the unit if the
/// supervisor dies, and can adopt it as main process once it is promoted.
pub(crate) struct CandidateProcess {
    process: ManagedProcess,
}

impl CandidateProcess {
    pub(crate) fn spawn(
        executable: &Executable,
        executable_root: &Path,
        instance_id: &str,
        candidate_address: SocketAddr,
        supervisor_address: SocketAddr,
    ) -> Result<Self, String> {
        let executable_path = executable.path(executable_root);
        let working_directory = executable.working_directory_path(executable_root);

        let mut command = Command::new(executable_path);

        command
            .current_dir(working_directory)
            .env("BCAIP_EXECUTABLE_ID", executable.id().to_string())
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

        Ok(Self {
            process: ManagedProcess::spawn(&mut command)?,
        })
    }

    pub(crate) fn pid(&self) -> u32 {
        self.process.pid()
    }

    pub(crate) fn terminate(self, timeout: Duration) -> Result<(), String> {
        self.process.terminate(timeout)
    }
}

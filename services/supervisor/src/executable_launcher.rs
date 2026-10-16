use crate::managed_process::ManagedProcess;
use executables_manager::Executable;
use executables_manager::ExecutableId;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

const STOP_GRACE: Duration = Duration::from_secs(5);

/// Starts and stops the executables the supervisor is responsible for
/// (`ExecutableStartPhase::Runtime`) as its own child processes.
/// Processes started by a previous supervisor (before a promotion) have no
/// `Child` here; they are stopped through their recorded pid.
pub(crate) struct ExecutableLauncher {
    executable_root: PathBuf,
    children: Mutex<HashMap<ExecutableId, ManagedProcess>>,
}

impl ExecutableLauncher {
    pub(crate) fn new(executable_root: &Path) -> Self {
        Self {
            executable_root: executable_root.to_path_buf(),
            children: Mutex::new(HashMap::new()),
        }
    }

    /// Replaces any running process of the executable by a new one.
    pub(crate) fn launch(
        &self,
        executable: &Executable,
        instance_id: &str,
        supervisor_address: SocketAddr,
    ) -> Result<(), String> {
        self.stop(executable)?;

        let executable_path = executable.path(&self.executable_root);
        let working_directory = executable.working_directory_path(&self.executable_root);

        let mut command = Command::new(executable_path);

        command
            .current_dir(working_directory)
            .env_remove("NOTIFY_SOCKET")
            .env_remove("BCAIP_SUPERVISOR_CANDIDATE")
            .env_remove("BCAIP_SUPERVISOR_BIND_ADDRESS")
            .env_remove("BCAIP_SUPERVISOR_CANDIDATE_BIND_ADDRESS")
            .env("BCAIP_EXECUTABLE_ID", executable.id().to_string())
            .env("BCAIP_EXECUTABLE_NAME", executable.name().as_str())
            .env("BCAIP_INSTANCE_ID", instance_id)
            .env(
                "BCAIP_WS_ADDRESS",
                executable.default_ws_address().to_string(),
            )
            .env("BCAIP_SUPERVISOR_ADDRESS", supervisor_address.to_string())
            .stdin(Stdio::null());

        if let Some(update_id) = executable.committed_update_id() {
            command.env("BCAIP_APPLIED_UPDATE_ID", update_id);
        }

        let process = ManagedProcess::spawn(&mut command)?;

        self.children
            .lock()
            .map_err(|_| "launcher lock poisoned".to_string())?
            .insert(executable.id(), process);

        Ok(())
    }

    /// Collects the children that exited so they do not stay zombies.
    pub(crate) fn reap(&self) {
        if let Ok(mut children) = self.children.lock() {
            children.retain(|_, process| !process.has_exited());
        }
    }

    /// Stops the executable's process, whether this supervisor started it or
    /// a previous one did. Returns only once it is gone.
    pub(crate) fn stop(&self, executable: &Executable) -> Result<(), String> {
        let owned = self
            .children
            .lock()
            .map_err(|_| "launcher lock poisoned".to_string())?
            .remove(&executable.id());

        match (owned, executable.process()) {
            (Some(process), _) => process.terminate(STOP_GRACE),

            (None, Some(inherited)) => ManagedProcess::terminate_inherited(
                inherited.pid(),
                &executable.path(&self.executable_root),
                STOP_GRACE,
            ),

            (None, None) => Ok(()),
        }
    }
}

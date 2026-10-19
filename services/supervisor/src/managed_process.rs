use rustix::process::{Pid, Signal};
use std::fs;
use std::path::Path;
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(50);
const KILL_TIMEOUT: Duration = Duration::from_secs(2);

/// A process started by this supervisor and owned through its `Child`.
///
/// Both ways of stopping a process (`terminate`, `terminate_inherited`) have
/// the same lifecycle: SIGTERM, wait up to `grace`, then SIGKILL and wait up
/// to `KILL_TIMEOUT` more. They only return `Ok` once the process is gone.
pub(crate) struct ManagedProcess {
    child: Child,
}

impl ManagedProcess {
    pub(crate) fn spawn(command: &mut Command) -> Result<Self, String> {
        let child = command
            .spawn()
            .map_err(|error| format!("failed to spawn {:?}: {error}", command.get_program()))?;

        Ok(Self { child })
    }

    pub(crate) fn pid(&self) -> u32 {
        self.child.id()
    }

    pub(crate) fn has_exited(&mut self) -> bool {
        !matches!(self.child.try_wait(), Ok(None))
    }

    pub(crate) fn terminate(mut self, grace: Duration) -> Result<(), String> {
        let pid = self.child.id();

        if self.has_exited() {
            return Ok(());
        }

        Self::signal(pid, Signal::TERM)?;

        if self.wait_for_exit(grace)? {
            return Ok(());
        }

        self.child
            .kill()
            .map_err(|error| format!("failed to kill process {pid}: {error}"))?;

        if self.wait_for_exit(KILL_TIMEOUT)? {
            return Ok(());
        }

        Err(format!("process {pid} is still running after SIGKILL"))
    }

    /// Stops a process this supervisor holds no `Child` for (started by a
    /// previous supervisor). The pid is only trusted while it still runs the
    /// expected binary, so a recycled pid is never signalled.
    pub(crate) fn terminate_inherited(
        pid: u32,
        expected_binary: &Path,
        grace: Duration,
    ) -> Result<(), String> {
        if !Self::is_running_binary(pid, expected_binary) {
            return Ok(());
        }

        Self::signal(pid, Signal::TERM)?;

        if Self::wait_until_gone(pid, expected_binary, grace) {
            return Ok(());
        }

        Self::signal(pid, Signal::KILL)?;

        if Self::wait_until_gone(pid, expected_binary, KILL_TIMEOUT) {
            return Ok(());
        }

        Err(format!("process {pid} is still running after SIGKILL"))
    }

    /// Whether a process with this pid exists, whatever it runs.
    pub(crate) fn pid_exists(pid: u32) -> bool {
        let Some(target) = i32::try_from(pid).ok().and_then(Pid::from_raw) else {
            return false;
        };

        match rustix::process::test_kill_process(target) {
            Ok(()) => true,
            Err(error) => error != rustix::io::Errno::SRCH,
        }
    }

    pub(crate) fn is_running_binary(pid: u32, expected_binary: &Path) -> bool {
        let Ok(running) = fs::read_link(format!("/proc/{pid}/exe")) else {
            return false;
        };

        // An updated binary replaces the file the process runs from.
        running == expected_binary
            || running.to_string_lossy() == format!("{} (deleted)", expected_binary.display())
    }

    fn wait_for_exit(&mut self, timeout: Duration) -> Result<bool, String> {
        let deadline = Instant::now() + timeout;

        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => return Ok(true),
                Ok(None) if Instant::now() >= deadline => return Ok(false),
                Ok(None) => thread::sleep(POLL_INTERVAL),
                Err(error) => {
                    return Err(format!(
                        "failed to wait for process {}: {error}",
                        self.child.id()
                    ));
                }
            }
        }
    }

    fn wait_until_gone(pid: u32, expected_binary: &Path, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;

        while Instant::now() < deadline {
            if !Self::is_running_binary(pid, expected_binary) {
                return true;
            }

            thread::sleep(POLL_INTERVAL);
        }

        !Self::is_running_binary(pid, expected_binary)
    }

    fn signal(pid: u32, signal: Signal) -> Result<(), String> {
        let target = i32::try_from(pid)
            .ok()
            .and_then(Pid::from_raw)
            .ok_or_else(|| format!("invalid pid {pid}"))?;

        rustix::process::kill_process(target, signal)
            .map_err(|error| format!("failed to send {signal:?} to process {pid}: {error}"))
    }
}

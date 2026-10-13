use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use uuid::Uuid;

use crate::managed_process::ManagedProcess;
use crate::{
    auto_updater::AutoUpdater, executable::Executable, executable_launcher::ExecutableLauncher,
    executable_store::ExecutableStore, monitoring_state::MonitoringState,
    supervisor_config::SupervisorConfig, ws_health::WsHealth,
};

const FAILURE_LIMIT: u32 = 3;

pub(crate) struct SupervisorMonitor {
    store: Arc<ExecutableStore>,
    config: SupervisorConfig,
    stop: Arc<AtomicBool>,
    supervisor_address: SocketAddr,
    state: MonitoringState,
    launcher: ExecutableLauncher,
}

impl SupervisorMonitor {
    pub(crate) fn new(
        store: Arc<ExecutableStore>,
        config: SupervisorConfig,
        stop: Arc<AtomicBool>,
        supervisor_address: SocketAddr,
    ) -> Self {
        Self {
            store,
            config,
            stop,
            supervisor_address,
            state: MonitoringState::new(FAILURE_LIMIT),
            launcher: ExecutableLauncher::new(),
        }
    }

    pub(crate) fn run(mut self) {
        if let Err(error) = self.reconcile_runtime_processes() {
            println!(
                "[Supervisor] failed to reconcile runtime processes: {}",
                error
            );
        }

        while !self.stop.load(Ordering::SeqCst) {
            match self.after_bcaip_environment_setup_is_ready() {
                Ok(true) => break,

                Ok(false) => {
                    thread::sleep(Duration::from_secs(1));
                }

                Err(error) => {
                    println!(
                        "[Supervisor] failed to check AfterBcaipEnvironmentSetup executables: {}",
                        error
                    );
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }

        if self.stop.load(Ordering::SeqCst) {
            return;
        }

        if let Err(error) = self.start_runtime_executables() {
            println!(
                "[Supervisor] failed to start Runtime executables: {}",
                error
            );
        }

        while !self.stop.load(Ordering::SeqCst) {
            let snapshot = match self.store.snapshot() {
                Ok(snapshot) => snapshot,

                Err(error) => {
                    println!("[Supervisor] failed to read executables: {}", error);
                    thread::sleep(Duration::from_secs(1));
                    continue;
                }
            };

            self.launcher.reap();

            if !self.supervise_once(&snapshot) {
                return;
            }

            thread::sleep(Duration::from_secs(1));
        }
    }

    fn supervise_once(&mut self, snapshot: &[Executable]) -> bool {
        for executable in snapshot {
            if self.config.is_current_supervisor(executable) {
                if !self.update_self_with_candidate(executable) {
                    return false;
                }

                continue;
            }

            if !executable.starts_at_runtime() {
                continue;
            }

            self.supervise_executable(executable);
        }

        true
    }

    fn after_bcaip_environment_setup_is_ready(&self) -> Result<bool, String> {
        for executable in self.store.snapshot()? {
            if executable.start_phase()
                != executables_manager::ExecutableStartPhase::AfterBcaipEnvironmentSetup
            {
                continue;
            }

            if !WsHealth::is_healthy(&executable) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn start_runtime_executables(&self) -> Result<(), String> {
        for executable in self.store.snapshot()? {
            if self.config.is_current_supervisor(&executable) {
                continue;
            }

            if !executable.starts_at_runtime() {
                continue;
            }

            if WsHealth::is_healthy(&executable) {
                continue;
            }

            self.restart_executable(&executable, false);
        }

        Ok(())
    }

    fn supervise_executable(&mut self, executable: &Executable) {
        let healthy = WsHealth::is_healthy(executable);
        let update_restart = executable.needs_update_restart();

        if healthy && !update_restart {
            self.state.reset_executable(executable.name());
            return;
        }

        if !update_restart && !self.state.executable_failed_enough(executable.name()) {
            return;
        }

        self.restart_executable(executable, update_restart);
        self.state.reset_executable(executable.name());
    }

    fn update_self_with_candidate(&self, executable: &Executable) -> bool {
        if !executable.needs_update_restart() {
            return true;
        }

        println!(
            "[Supervisor] validating {} candidate after committed update",
            executable.name()
        );

        AutoUpdater::new(
            Arc::clone(&self.store),
            self.config.clone(),
            self.supervisor_address,
            Arc::clone(&self.stop),
        )
        .update_supervisor(executable)
    }

    fn restart_executable(&self, executable: &Executable, update_restart: bool) {
        println!(
            "[Supervisor] restarting {}{}",
            executable.name(),
            if update_restart {
                " after committed update"
            } else {
                ""
            }
        );

        let process_instance_id = Uuid::now_v7().to_string();

        if let Err(error) = self
            .store
            .expect_process(executable.name(), &process_instance_id)
        {
            println!(
                "[Supervisor] failed to reserve {} process instance: {}",
                executable.name(),
                error
            );
            return;
        }

        if let Err(error) =
            self.launcher
                .launch(executable, &process_instance_id, self.supervisor_address)
        {
            println!(
                "[Supervisor] failed to restart {}: {}",
                executable.name(),
                error
            );
        }
    }

    /// Drops the persisted `Process` of an executable only once its process
    /// is gone: a recorded pid is what lets a still-living process (started
    /// by a previous supervisor) be stopped before a new one is launched.
    fn reconcile_runtime_processes(&self) -> Result<(), String> {
        for executable in self.store.snapshot()? {
            if self.config.is_current_supervisor(&executable) {
                continue;
            }

            let Some(process) = executable.process() else {
                continue;
            };

            if WsHealth::is_healthy(&executable) {
                continue;
            }

            if executable.starts_at_runtime() {
                if let Err(error) = self.launcher.stop(&executable) {
                    println!(
                        "[Supervisor] failed to stop previous {} process, keeping its record: {}",
                        executable.name(),
                        error
                    );

                    continue;
                }
            } else if ManagedProcess::is_running_binary(process.pid(), &executable.path()) {
                // Not started by the supervisor, so not stopped by it either.
                continue;
            }

            self.store
                .clear_process(executable.name(), process.instance_id())?;
        }

        Ok(())
    }
}

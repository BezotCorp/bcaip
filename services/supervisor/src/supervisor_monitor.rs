use crate::auto_updater::AutoUpdater;
use crate::executable::Executable;
use crate::executable_store::ExecutableStore;
use crate::monitoring_state::MonitoringState;
use crate::supervisor_config::SupervisorConfig;
use crate::systemd::Systemd;
use crate::ws_health::WsHealth;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use uuid::Uuid;

const FAILURE_LIMIT: u32 = 3;

pub(crate) struct SupervisorMonitor {
    store: Arc<ExecutableStore>,
    config: SupervisorConfig,
    stop: Arc<AtomicBool>,
    supervisor_address: SocketAddr,
    state: MonitoringState,
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
            let snapshot = match self.store.snapshot() {
                Ok(snapshot) => snapshot,

                Err(error) => {
                    println!("[Supervisor] failed to read executables: {}", error);
                    thread::sleep(Duration::from_secs(1));
                    continue;
                }
            };

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

            self.supervise_executable(executable);
        }

        true
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
                " after WebSocket health failure"
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
            Systemd::restart_transient(executable, &process_instance_id, self.supervisor_address)
        {
            println!(
                "[Supervisor] failed to restart {}: {}",
                executable.name(),
                error
            );
        }
    }

    fn reconcile_runtime_processes(&self) -> Result<(), String> {
        for executable in self.store.snapshot()? {
            if self.config.is_current_supervisor(&executable) {
                continue;
            }

            let Some(instance_id) = executable.process_instance_id() else {
                continue;
            };

            if WsHealth::is_healthy(&executable) {
                continue;
            }

            self.store.clear_process(executable.name(), instance_id)?;
        }

        Ok(())
    }
}

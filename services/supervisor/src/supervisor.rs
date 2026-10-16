use crate::executable_store::ExecutableStore;
use crate::handover_recovery::HandoverRecovery;
use crate::supervisor_config::SupervisorConfig;
use crate::supervisor_monitor::SupervisorMonitor;
use crate::systemd_notifier::SystemdNotifier;
use crate::update_server::UpdateServer;
use executables_manager::AuthorityState;
use executables_manager::Executable;
use executables_manager::Process;
use std::error::Error;
use std::io;
use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use uuid::Uuid;
use ws_manager::WsServer;

pub(crate) struct Supervisor {
    config: SupervisorConfig,
    store: Arc<ExecutableStore>,
    listener: TcpListener,
    actual_address: SocketAddr,
    instance_id: String,
    applied_update_id: Option<String>,
    self_definition: Option<Executable>,
    monitor: Option<thread::JoinHandle<()>>,
    stop: Arc<AtomicBool>,
}

impl Supervisor {
    pub(crate) fn from_env() -> Result<Self, Box<dyn Error>> {
        let config = SupervisorConfig::from_env()?;
        let authority = if config.is_candidate() {
            AuthorityState::candidate(config.data_directory())
        } else {
            AuthorityState::writer(config.data_directory())?
        };
        let mut recovery = match authority.can_write() {
            true => HandoverRecovery::detect(config.data_directory(), config.executable_root())?,
            false => None,
        };

        if let Some(recovery) = recovery.as_mut() {
            recovery.restore_runtime_state()?;
        }

        let store = Arc::new(ExecutableStore::load(config.data_directory(), authority)?);

        if let Some(recovery) = recovery {
            if recovery.conclude(&store)? {
                return Err("auto-update recovery restored the previous binary; restarting".into());
            }
        }

        if store.can_write() {
            UpdateServer::reconcile_all(&store, config.executable_root())?;
        }

        let self_definition = store.get(config.supervisor_name())?;

        let bind_address = config.bind_address(self_definition.as_ref())?;
        let listener = TcpListener::bind(bind_address)?;
        listener.set_nonblocking(true)?;

        let actual_address = listener.local_addr()?;
        let instance_id =
            std::env::var("BCAIP_INSTANCE_ID").unwrap_or_else(|_| Uuid::now_v7().to_string());
        let applied_update_id = self_definition
            .as_ref()
            .and_then(Executable::committed_update_id)
            .map(str::to_string);
        let stop = Arc::new(AtomicBool::new(false));

        Ok(Self {
            config,
            store,
            listener,
            actual_address,
            instance_id,
            applied_update_id,
            self_definition,
            monitor: None,
            stop,
        })
    }

    pub(crate) fn run(mut self) -> Result<(), Box<dyn Error>> {
        self.install_signal_handler()?;
        self.register_self()?;
        self.log_start();

        if self.store.can_write() {
            self.monitor = Some(self.spawn_monitor());
            SystemdNotifier::ready()?;
        }

        let result = self.accept_connections();

        self.stop.store(true, Ordering::SeqCst);

        if let Some(monitor) = self.monitor.take() {
            let _ = monitor.join();
        }
        self.unregister_self();

        result
    }

    fn install_signal_handler(&self) -> Result<(), Box<dyn Error>> {
        let signal_stop = Arc::clone(&self.stop);

        ctrlc::set_handler(move || {
            signal_stop.store(true, Ordering::SeqCst);
        })?;

        Ok(())
    }

    fn register_self(&self) -> Result<(), String> {
        if self.self_definition.is_some() && self.store.can_write() {
            self.store
                .expect_process(self.config.supervisor_name(), &self.instance_id)?;
            self.store.set_process(
                self.config.supervisor_name(),
                Process::new(
                    std::process::id(),
                    self.instance_id.clone(),
                    self.actual_address,
                    self.applied_update_id.clone(),
                ),
            )?;
        }

        Ok(())
    }

    fn unregister_self(&self) {
        if self.self_definition.is_some() && self.store.can_write() {
            let _ = self
                .store
                .clear_process(self.config.supervisor_name(), &self.instance_id);
        }
    }

    fn spawn_monitor(&self) -> thread::JoinHandle<()> {
        let monitor = SupervisorMonitor::new(
            Arc::clone(&self.store),
            self.config.clone(),
            Arc::clone(&self.stop),
            self.actual_address,
        );

        thread::spawn(move || monitor.run())
    }

    fn accept_connections(&mut self) -> Result<(), Box<dyn Error>> {
        while !self.stop.load(Ordering::SeqCst) {
            self.take_over_promoted_address()?;

            match self.listener.accept() {
                Ok((stream, peer)) => {
                    let connection_store = Arc::clone(&self.store);
                    let connection_config = self.config.clone();
                    let connection_instance_id = self.instance_id.clone();
                    let connection_applied_update_id = self.applied_update_id.clone();

                    thread::spawn(move || {
                        let ws_server = WsServer::new(
                            Some(Duration::from_secs(30)),
                            Some(Duration::from_secs(30)),
                        );

                        let result = ws_server.accept(stream).and_then(|connection| {
                            let update_server = UpdateServer::new(
                                &connection_store,
                                &connection_config,
                                &connection_instance_id,
                                connection_applied_update_id.as_deref(),
                            );

                            update_server.handle_connection(connection)
                        });

                        if let Err(error) = result {
                            println!("[Supervisor] connection from {} failed: {}", peer, error);
                        }
                    });
                }

                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(50));
                }

                Err(error) => {
                    return Err(error.into());
                }
            }
        }

        Ok(())
    }

    /// A promoted candidate is the writer but still listens on the candidate
    /// address. Once the previous supervisor has released its own address,
    /// serve it and start supervising.
    fn take_over_promoted_address(&mut self) -> Result<(), Box<dyn Error>> {
        if self.monitor.is_some() || !self.store.can_write() {
            return Ok(());
        }

        let Some(address) = self.config.promoted_address() else {
            return Ok(());
        };

        match TcpListener::bind(address) {
            Ok(listener) => {
                listener.set_nonblocking(true)?;
                self.actual_address = listener.local_addr()?;
                self.listener = listener;
                UpdateServer::reconcile_all(&self.store, self.config.executable_root())?;
                self.monitor = Some(self.spawn_monitor());
                HandoverRecovery::confirm_promotion(
                    self.config.data_directory(),
                    self.config.executable_root(),
                    &self.store,
                )?;

                println!(
                    "[Supervisor] promoted candidate is now authoritative on {}",
                    self.actual_address
                );

                Ok(())
            }

            Err(error) if error.kind() == io::ErrorKind::AddrInUse => Ok(()),

            Err(error) => Err(error.into()),
        }
    }

    fn log_start(&self) {
        println!(
            "[Supervisor] {} {:?} listening on {} using {}",
            self.config.supervisor_name(),
            if self.store.can_write() {
                "authoritative"
            } else {
                "candidate"
            },
            self.actual_address,
            self.store.source_path().display(),
        );
    }
}

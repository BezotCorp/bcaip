use crate::health_check::HealthCheck;
use crate::service_config::ServiceConfig;
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use std::{io, thread};
use tungstenite::{Message, client};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const PROBE_INTERVAL: Duration = Duration::from_secs(3);
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_FAILURES: u32 = 3;

#[derive(Clone, Debug)]
pub(crate) struct LocalService {
    service: ServiceConfig,
    server_root: PathBuf,
    executable_path: PathBuf,
}

impl LocalService {
    pub(crate) fn new(
        service: ServiceConfig,
        server_root: PathBuf,
        executable_path: PathBuf,
    ) -> Self {
        Self {
            service,
            server_root,
            executable_path,
        }
    }

    pub(crate) fn monitor(&self, stop: &AtomicBool) {
        let mut failures_in_row = 0u32;

        while !stop.load(Ordering::SeqCst) {
            if !self.executable_path.is_file() {
                eprintln!(
                    "[LocalLauncher] executable not found for {}: {}",
                    self.service.name,
                    self.executable_path.display()
                );
                failures_in_row = failures_in_row.saturating_add(1);
                sleep_interruptible(stop, restart_delay(failures_in_row));
                continue;
            }

            let mut child = match Command::new(&self.executable_path)
                .current_dir(&self.server_root)
                .stdin(Stdio::null())
                .spawn()
            {
                Ok(child) => child,
                Err(error) => {
                    eprintln!(
                        "[LocalLauncher] failed to start {}: {error}",
                        self.service.name
                    );
                    failures_in_row = failures_in_row.saturating_add(1);
                    sleep_interruptible(stop, restart_delay(failures_in_row));
                    continue;
                }
            };

            eprintln!(
                "[LocalLauncher] {} started as PID {}",
                self.service.name,
                child.id()
            );

            let started = Instant::now();
            let mut healthy = false;
            let mut probe_failures = 0;

            while !stop.load(Ordering::SeqCst) {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        eprintln!("[LocalLauncher] {} exited: {}", self.service.name, status);
                        break;
                    }
                    Ok(None) => {}
                    Err(error) => {
                        eprintln!(
                            "[LocalLauncher] failed to query {} process state: {error}",
                            self.service.name
                        );
                        break;
                    }
                }

                if self.service.health_check.is_healthy() {
                    if !healthy {
                        eprintln!("[LocalLauncher] {} is ready", self.service.name);
                    }

                    healthy = true;
                    probe_failures = 0;
                } else if healthy {
                    probe_failures += 1;

                    eprintln!(
                        "[LocalLauncher] {} health failure {}/{}",
                        self.service.name, probe_failures, MAX_FAILURES
                    );

                    if probe_failures >= MAX_FAILURES {
                        eprintln!("[LocalLauncher] {} unhealthy; restarting", self.service.name);
                        break;
                    }
                } else if started.elapsed() >= STARTUP_TIMEOUT {
                    eprintln!(
                        "[LocalLauncher] {} startup timeout; restarting",
                        self.service.name
                    );
                    break;
                }

                sleep_interruptible(stop, PROBE_INTERVAL);
            }

            if let Err(error) = terminate_child(&mut child) {
                eprintln!(
                    "[LocalLauncher] failed to terminate {}: {error}",
                    self.service.name
                );
            }

            if stop.load(Ordering::SeqCst) {
                break;
            }

            if started.elapsed() >= Duration::from_secs(60) {
                failures_in_row = 0;
            }

            failures_in_row = failures_in_row.saturating_add(1);
            let delay = restart_delay(failures_in_row);

            eprintln!(
                "[LocalLauncher] Restarting {} in {}s",
                self.service.name,
                delay.as_secs()
            );

            sleep_interruptible(stop, delay);
        }
    }
}

impl HealthCheck {
    pub(crate) fn is_healthy(&self) -> bool {
        match self {
            Self::WebSocketPing { address } => ws_ping(*address),
        }
    }
}

fn ws_ping(address: SocketAddr) -> bool {
    let Ok(stream) = TcpStream::connect_timeout(&address, PROBE_TIMEOUT) else {
        return false;
    };

    if stream.set_read_timeout(Some(PROBE_TIMEOUT)).is_err()
        || stream.set_write_timeout(Some(PROBE_TIMEOUT)).is_err()
    {
        return false;
    }

    let Ok((mut socket, _)) = client(format!("ws://{address}/"), stream) else {
        return false;
    };

    if socket.send(Message::Ping(Vec::new().into())).is_err() {
        return false;
    }

    loop {
        match socket.read() {
            Ok(Message::Pong(payload)) if payload.is_empty() => {
                return true;
            }

            Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {
                continue;
            }

            Ok(_) | Err(_) => {
                return false;
            }
        }
    }
}

fn restart_delay(failures_in_row: u32) -> Duration {
    Duration::from_secs((2_u64.saturating_pow(failures_in_row.min(5))).min(30))
}

fn sleep_interruptible(stop: &AtomicBool, duration: Duration) {
    let end = Instant::now() + duration;

    while !stop.load(Ordering::SeqCst) && Instant::now() < end {
        thread::sleep(
            end.saturating_duration_since(Instant::now())
                .min(Duration::from_millis(100)),
        );
    }
}

fn terminate_child(child: &mut Child) -> io::Result<()> {
    if child.try_wait()?.is_some() {
        return Ok(());
    }

    child.kill()?;
    child.wait()?;
    Ok(())
}

mod begin_update_result;
mod executable;
mod executable_store;
mod process;
mod systemd;
mod update;
mod update_phase;
mod update_server;
mod watcher_request;
mod watcher_response;
mod ws_health;

use crate::executable::Executable;
use crate::executable_store::ExecutableStore;
use crate::process::Process;
use crate::update_phase::UpdatePhase;
use crate::update_server::{handle_connection, reconcile_all};
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use uuid::Uuid;

const DEFAULT_LAUNCHER_NAME: &str = "local_launcher";
const DEFAULT_WATCHER_NAME: &str = "watcher";
const DEFAULT_BIND_ADDRESS: &str = "127.0.0.1:3010";
const FAILURE_LIMIT: u32 = 3;

fn main() -> Result<(), Box<dyn Error>> {
    let stop = Arc::new(AtomicBool::new(false));
    let signal_stop = Arc::clone(&stop);

    ctrlc::set_handler(move || {
        signal_stop.store(true, Ordering::SeqCst);
    })?;

    let data_directory = data_directory()?;

    let store = Arc::new(ExecutableStore::load(&data_directory)?);

    reconcile_all(&store)?;

    let launcher_name =
        env::var("BCAIP_LAUNCHER_NAME").unwrap_or_else(|_| DEFAULT_LAUNCHER_NAME.to_string());

    let watcher_name =
        env::var("BCAIP_WATCHER_NAME").unwrap_or_else(|_| DEFAULT_WATCHER_NAME.to_string());

    let self_definition = store.get(&watcher_name)?;

    let bind_address = match self_definition.as_ref() {
        Some(executable) => executable.default_ws_address,
        None => env::var("BCAIP_WATCHER_BIND_ADDRESS")
            .unwrap_or_else(|_| DEFAULT_BIND_ADDRESS.to_string())
            .parse::<SocketAddr>()?,
    };

    let listener = std::net::TcpListener::bind(bind_address)?;
    listener.set_nonblocking(true)?;

    let actual_address = listener.local_addr()?;

    let instance_id = env::var("BCAIP_INSTANCE_ID").unwrap_or_else(|_| Uuid::now_v7().to_string());

    let self_applied_update_id = self_definition.as_ref().and_then(committed_update_id);

    if self_definition.is_some() {
        store.expect_process(&watcher_name, &instance_id)?;

        store.set_process(
            &watcher_name,
            Process {
                pid: std::process::id(),
                instance_id: instance_id.clone(),
                ws_address: actual_address,
                applied_update_id: self_applied_update_id.clone(),
            },
        )?;
    }

    println!(
        "[Watcher] listening on {} using {}",
        actual_address,
        store.source_path().display(),
    );

    let monitor_store = Arc::clone(&store);
    let monitor_stop = Arc::clone(&stop);
    let monitor_launcher_name = launcher_name.clone();
    let monitor_watcher_name = watcher_name.clone();

    let monitor_watcher_address = actual_address;

    let monitor_handle = thread::spawn(move || {
        if let Err(error) = reconcile_runtime_processes(&monitor_store, &monitor_watcher_name) {
            println!("[Watcher] failed to reconcile runtime processes: {}", error);
        }

        monitor(
            monitor_store,
            &monitor_stop,
            &monitor_launcher_name,
            &monitor_watcher_name,
            monitor_watcher_address,
        );
    });

    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, peer)) => {
                let connection_store = Arc::clone(&store);

                let connection_watcher_name = watcher_name.clone();

                let connection_instance_id = instance_id.clone();

                let connection_applied_update_id = self_applied_update_id.clone();

                thread::spawn(move || {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));

                    let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));

                    if let Err(error) = handle_connection(
                        stream,
                        &connection_store,
                        &connection_watcher_name,
                        &connection_instance_id,
                        connection_applied_update_id.as_deref(),
                    ) {
                        println!("[Watcher] connection from {} failed: {}", peer, error);
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

    let _ = monitor_handle.join();

    if self_definition.is_some() {
        let _ = store.clear_process(&watcher_name, &instance_id);
    }

    Ok(())
}

fn monitor(
    store: Arc<ExecutableStore>,
    stop: &AtomicBool,
    launcher_name: &str,
    watcher_name: &str,
    watcher_address: SocketAddr,
) {
    let mut failures: HashMap<String, u32> = HashMap::new();

    let mut launcher_restart_attempted = false;

    while !stop.load(Ordering::SeqCst) {
        let snapshot = match store.snapshot() {
            Ok(snapshot) => snapshot,

            Err(error) => {
                println!("[Watcher] failed to read executables: {}", error);

                thread::sleep(Duration::from_secs(1));

                continue;
            }
        };

        let launcher = snapshot
            .iter()
            .find(|executable| executable.name == launcher_name);

        let Some(launcher) = launcher else {
            println!("[Watcher] launcher definition is missing");

            thread::sleep(Duration::from_secs(1));

            continue;
        };

        let launcher_healthy = ws_health::is_healthy(launcher);

        let launcher_update_restart = needs_update_restart(launcher);

        let launcher_failure = {
            let failure = failures.entry(launcher.name.clone()).or_default();

            if launcher_healthy {
                *failure = 0;
                launcher_restart_attempted = false;
            } else {
                *failure = failure.saturating_add(1);
            }

            *failure
        };

        /*
         * Update or first health-failure window:
         * recover launcher first.
         *
         * restart() is blocking for peer persistent units, so when it
         * returns the old launcher has been stopped and systemd has
         * processed the restart job.
         */
        if launcher_update_restart
            || (!launcher_healthy
                && launcher_failure >= FAILURE_LIMIT
                && !launcher_restart_attempted)
        {
            println!(
                "[Watcher] restarting launcher {}{}",
                launcher.name,
                if launcher_update_restart {
                    " after committed update"
                } else {
                    " after WebSocket health failure"
                }
            );

            let previous_instance_id = launcher
                .process
                .as_ref()
                .map(|process| process.instance_id.clone());

            match systemd::restart_persistent(&launcher.name, launcher_name, watcher_name) {
                Ok(()) => {
                    launcher_restart_attempted = true;

                    if let Some(instance_id) = previous_instance_id {
                        if let Err(error) = store.clear_process(&launcher.name, &instance_id) {
                            println!(
                                "[Watcher] failed to clear previous launcher process: {}",
                                error
                            );
                        }
                    }
                }

                Err(error) => {
                    println!("[Watcher] launcher restart failed: {}", error);

                    launcher_restart_attempted = true;
                }
            }

            failures.insert(launcher.name.clone(), 0);

            thread::sleep(Duration::from_secs(1));

            continue;
        }

        /*
         * We always probe every executable.
         * Restart authority is decided separately.
         */
        for executable in &snapshot {
            if executable.name == launcher_name {
                continue;
            }

            let healthy = if executable.name == watcher_name {
                true
            } else {
                ws_health::is_healthy(executable)
            };

            let update_restart = needs_update_restart(executable);

            let failure = failures.entry(executable.name.clone()).or_default();

            if healthy {
                *failure = 0;
            } else {
                *failure = failure.saturating_add(1);
            }

            /*
             * Healthy launcher = watcher observes only.
             */
            if launcher_healthy {
                continue;
            }

            /*
             * Launcher restart was requested. Give the new launcher
             * one complete failure window to become functional before
             * fallback authority activates.
             */
            if !launcher_restart_attempted || launcher_failure < FAILURE_LIMIT {
                continue;
            }

            if executable.name == watcher_name {
                if update_restart {
                    println!("[Watcher] restarting itself after committed update");

                    if let Err(error) = systemd::restart_self() {
                        println!("[Watcher] self restart failed: {}", error);
                    }

                    return;
                }

                continue;
            }

            if !update_restart && *failure < FAILURE_LIMIT {
                continue;
            }

            println!(
                "[Watcher] fallback restart of {}{}",
                executable.name,
                if update_restart {
                    " after committed update"
                } else {
                    " after WebSocket health failure"
                }
            );

            let process_instance_id = Uuid::now_v7().to_string();

            if let Err(error) = store.expect_process(&executable.name, &process_instance_id) {
                println!(
                    "[Watcher] failed to reserve {} process instance: {}",
                    executable.name, error
                );

                *failure = 0;
                continue;
            }

            if let Err(error) =
                systemd::restart_transient(executable, &process_instance_id, watcher_address)
            {
                println!("[Watcher] failed to restart {}: {}", executable.name, error);
            }

            *failure = 0;
        }

        thread::sleep(Duration::from_secs(1));
    }
}

fn needs_update_restart(executable: &Executable) -> bool {
    let Some(update_id) = committed_update_id(executable) else {
        return false;
    };

    let Some(process) = executable.process.as_ref() else {
        return false;
    };

    process.applied_update_id.as_deref() != Some(update_id.as_str())
}

fn committed_update_id(executable: &Executable) -> Option<String> {
    executable
        .update
        .as_ref()
        .filter(|update| update.phase == UpdatePhase::Committed)
        .map(|update| update.update_id.clone())
}

fn runtime_ws_address(executable: &Executable) -> SocketAddr {
    executable
        .process
        .as_ref()
        .map(|process| process.ws_address)
        .unwrap_or(executable.default_ws_address)
}

fn reconcile_runtime_processes(store: &ExecutableStore, watcher_name: &str) -> Result<(), String> {
    for executable in store.snapshot()? {
        if executable.name == watcher_name {
            continue;
        }

        let Some(process) = executable.process.as_ref() else {
            continue;
        };

        if ws_health::is_healthy(&executable) {
            continue;
        }

        store.clear_process(&executable.name, &process.instance_id)?;
    }

    Ok(())
}

fn data_directory() -> Result<PathBuf, Box<dyn Error>> {
    if let Some(directory) = env::var_os("BCAIP_DATA_DIR") {
        return Ok(PathBuf::from(directory));
    }

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let root = manifest
        .parent()
        .and_then(|services| services.parent())
        .ok_or("watcher manifest has no workspace root")?;

    Ok(root.join("data"))
}

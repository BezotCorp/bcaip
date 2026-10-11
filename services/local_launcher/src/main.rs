mod executable;
mod executable_data;
mod process;
mod systemd;
mod update;
mod update_phase;
mod watcher_client;
mod watcher_request;
mod watcher_response;
mod ws_health;

use crate::executable::Executable;
use crate::executable_data::{load_definitions, load_runtime};
use crate::update_phase::UpdatePhase;
use crate::watcher_request::WatcherRequest;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use uuid::Uuid;

const DEFAULT_LAUNCHER_NAME: &str = "local_launcher";

const DEFAULT_WATCHER_NAME: &str = "watcher";

const DEFAULT_BIND_ADDRESS: &str = "127.0.0.1:3009";

const FAILURE_LIMIT: u32 = 3;

fn main() -> Result<(), Box<dyn Error>> {
    let stop = Arc::new(AtomicBool::new(false));

    let signal_stop = Arc::clone(&stop);

    ctrlc::set_handler(move || {
        signal_stop.store(true, Ordering::SeqCst);
    })?;

    let data_directory = data_directory()?;

    let definitions = load_definitions(&data_directory)?;

    let launcher_name =
        env::var("BCAIP_LAUNCHER_NAME").unwrap_or_else(|_| DEFAULT_LAUNCHER_NAME.to_string());

    let watcher_name =
        env::var("BCAIP_WATCHER_NAME").unwrap_or_else(|_| DEFAULT_WATCHER_NAME.to_string());

    let self_definition = definitions
        .iter()
        .find(|executable| executable.name == launcher_name)
        .cloned();

    if !definitions
        .iter()
        .any(|executable| executable.name == watcher_name)
    {
        return Err(format!(
            "watcher executable {} is missing from executable definitions",
            watcher_name
        )
        .into());
    }

    let bind_address = match self_definition.as_ref() {
        Some(executable) => executable.default_ws_address,

        None => env::var("BCAIP_LOCAL_LAUNCHER_BIND_ADDRESS")
            .unwrap_or_else(|_| DEFAULT_BIND_ADDRESS.to_string())
            .parse::<SocketAddr>()?,
    };

    let listener = ws_health::bind(bind_address)?;

    let actual_address = listener.local_addr()?;

    let instance_id = env::var("BCAIP_INSTANCE_ID").unwrap_or_else(|_| Uuid::now_v7().to_string());

    let self_applied_update_id = runtime_executable(&data_directory, &launcher_name)
        .and_then(|executable| committed_update_id(&executable));

    let ws_stop = Arc::clone(&stop);

    let ws_executable = launcher_name.clone();

    let ws_instance_id = instance_id.clone();

    let ws_applied_update_id = self_applied_update_id.clone();

    let ws_handle = thread::spawn(move || {
        ws_health::serve(
            listener,
            &ws_stop,
            ws_executable,
            ws_instance_id,
            ws_applied_update_id,
        );
    });

    println!("[LocalLauncher] listening on {}", actual_address);

    let mut failures: HashMap<String, u32> = HashMap::new();

    while !stop.load(Ordering::SeqCst) {
        let runtime = runtime_snapshot(&data_directory, &definitions);

        let watcher = runtime
            .iter()
            .find(|executable| executable.name == watcher_name);

        let Some(watcher) = watcher else {
            return Err("watcher disappeared from runtime definitions".into());
        };

        let watcher_address = runtime_ws_address(watcher);

        let watcher_healthy = ws_health::is_healthy(watcher);

        let watcher_update_restart = needs_update_restart(watcher);

        let watcher_failure = failures.entry(watcher_name.clone()).or_default();

        if watcher_healthy {
            *watcher_failure = 0;
        } else {
            *watcher_failure = watcher_failure.saturating_add(1);
        }

        if watcher_update_restart || *watcher_failure >= FAILURE_LIMIT {
            println!(
                "[LocalLauncher] restarting watcher {}{}",
                watcher.name,
                if watcher_update_restart {
                    " after committed update"
                } else {
                    " after WebSocket health failure"
                }
            );

            if let Err(error) =
                systemd::restart_persistent(&watcher.name, &launcher_name, &watcher_name)
            {
                println!("[LocalLauncher] watcher restart failed: {}", error);
            }

            *watcher_failure = 0;

            thread::sleep(Duration::from_secs(1));

            continue;
        }

        /*
         * Watcher owns executables.bin.
         * Do not create a process that cannot be registered.
         */
        if !watcher_healthy {
            thread::sleep(Duration::from_secs(1));

            continue;
        }

        if let Err(error) = watcher_client::acknowledge(
            watcher_address,
            &WatcherRequest::ProcessStarted {
                executable: launcher_name.clone(),
                pid: std::process::id(),
                instance_id: instance_id.clone(),
                ws_address: actual_address,
                applied_update_id: self_applied_update_id.clone(),
            },
        ) {
            println!("[LocalLauncher] watcher registration failed: {}", error);

            thread::sleep(Duration::from_secs(1));

            continue;
        }

        for definition in &definitions {
            if definition.name == launcher_name || definition.name == watcher_name {
                continue;
            }

            let executable = runtime
                .iter()
                .find(|candidate| candidate.name == definition.name)
                .unwrap_or(definition);

            let healthy = ws_health::is_healthy(executable);

            let update_restart = needs_update_restart(executable);

            if healthy && !update_restart {
                failures.insert(executable.name.clone(), 0);

                continue;
            }

            let should_restart = if update_restart {
                true
            } else {
                let failure = failures.entry(executable.name.clone()).or_default();

                *failure = failure.saturating_add(1);

                *failure >= FAILURE_LIMIT
            };

            if !should_restart {
                continue;
            }

            println!(
                "[LocalLauncher] restarting {}{}",
                executable.name,
                if update_restart {
                    " after committed update"
                } else {
                    " after WebSocket health failure"
                }
            );

            let process_instance_id = Uuid::now_v7().to_string();

            if let Err(error) = watcher_client::acknowledge(
                watcher_address,
                &WatcherRequest::ExpectProcess {
                    executable: executable.name.clone(),
                    instance_id: process_instance_id.clone(),
                },
            ) {
                println!(
                    "[LocalLauncher] failed to reserve {} process instance: {}",
                    executable.name, error
                );

                failures.insert(executable.name.clone(), 0);

                continue;
            }

            if let Err(error) =
                systemd::restart_transient(executable, &process_instance_id, watcher_address)
            {
                println!(
                    "[LocalLauncher] failed to restart {}: {}",
                    executable.name, error
                );
            }

            failures.insert(executable.name.clone(), 0);
        }

        thread::sleep(Duration::from_secs(1));
    }

    let runtime = runtime_snapshot(&data_directory, &definitions);

    if let Some(watcher_address) = current_watcher_address(&runtime, &watcher_name) {
        let _ = watcher_client::acknowledge(
            watcher_address,
            &WatcherRequest::ProcessStopped {
                executable: launcher_name,
                instance_id,
            },
        );
    }

    let _ = ws_handle.join();

    Ok(())
}

fn current_watcher_address(runtime: &[Executable], watcher_name: &str) -> Option<SocketAddr> {
    runtime
        .iter()
        .find(|executable| executable.name == watcher_name)
        .map(runtime_ws_address)
}

fn runtime_snapshot(data_directory: &Path, definitions: &[Executable]) -> Vec<Executable> {
    load_runtime(data_directory).unwrap_or_else(|_| definitions.to_vec())
}

fn runtime_executable(data_directory: &Path, name: &str) -> Option<Executable> {
    load_runtime(data_directory)
        .ok()?
        .into_iter()
        .find(|executable| executable.name == name)
}

fn runtime_ws_address(executable: &Executable) -> SocketAddr {
    executable
        .process
        .as_ref()
        .map(|process| process.ws_address)
        .unwrap_or(executable.default_ws_address)
}

fn committed_update_id(executable: &Executable) -> Option<String> {
    executable
        .update
        .as_ref()
        .filter(|update| update.phase == UpdatePhase::Committed)
        .map(|update| update.update_id.clone())
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

fn data_directory() -> Result<PathBuf, Box<dyn Error>> {
    if let Some(directory) = env::var_os("BCAIP_DATA_DIR") {
        return Ok(PathBuf::from(directory));
    }

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let root = manifest
        .parent()
        .and_then(|services| services.parent())
        .ok_or("local launcher manifest has no workspace root")?;

    Ok(root.join("data"))
}

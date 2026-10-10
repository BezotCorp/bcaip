mod extension;
mod file;
mod files;
mod health_check;
mod id;
mod launcher_cache;
mod launcher_config;
mod local_service;
mod server_config;
mod server_id;
mod server_location;
mod service_config;
mod watcher_update_phase;
mod watcher_update_record;

use crate::watcher_update_record::WatcherUpdateRecord;
use files::Files;
use std::{
    env,
    error::Error,
    path::PathBuf,
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};

const DEFAULT_SELF_UPDATE_PRODUCT: &str = "local_launcher";

fn main() -> Result<(), Box<dyn Error>> {
    let stop = Arc::new(AtomicBool::new(false));
    let signal_stop = Arc::clone(&stop);

    ctrlc::set_handler(move || {
        signal_stop.store(true, Ordering::SeqCst);
    })?;

    let launcher_directory = match env::var_os("BCAIP_LOCAL_LAUNCHER_CONFIG_DIR") {
        Some(directory) => PathBuf::from(directory),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    };

    let watcher_state_directory = match env::var_os("BCAIP_WATCHER_STATE_DIR") {
        Some(directory) => PathBuf::from(directory),

        None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(|services| services.join("watcher").join("state"))
            .ok_or("local launcher has no services parent directory")?,
    };

    let self_update_product = env::var("BCAIP_LOCAL_LAUNCHER_UPDATE_PRODUCT")
        .unwrap_or_else(|_| DEFAULT_SELF_UPDATE_PRODUCT.to_string());

    let files = Files::from_launcher_directory(&launcher_directory)?;
    let launcher_config = files.load_launcher_configuration()?;

    let services = launcher_config
        .local_services(&watcher_state_directory)?;

    println!(
        "[LocalLauncher] Supervising {} service(s)",
        services.len()
    );

    let self_update_stop = Arc::clone(&stop);
    let self_update_state_directory = watcher_state_directory.clone();

    let self_update_handle = thread::spawn(move || {
        monitor_self_update(
            &self_update_stop,
            &self_update_state_directory,
            &self_update_product,
        );
    });

    let handles: Vec<_> = services
        .into_iter()
        .map(|service| {
            let thread_stop = Arc::clone(&stop);

            thread::spawn(move || {
                let result = service.monitor(&thread_stop);

                if let Err(error) = &result {
                    eprintln!(
                        "[LocalLauncher] Fatal supervisor error: {}",
                        error
                    );

                    thread_stop.store(true, Ordering::SeqCst);
                }

                result
            })
        })
        .collect();

    let mut failed = false;

    for handle in handles {
        match handle.join() {
            Ok(Ok(())) => {}

            Ok(Err(error)) => {
                eprintln!(
                    "[LocalLauncher] Supervisor failed: {}",
                    error
                );

                failed = true;
            }

            Err(_) => {
                eprintln!("[LocalLauncher] Supervisor panicked");
                stop.store(true, Ordering::SeqCst);
                failed = true;
            }
        }
    }

    stop.store(true, Ordering::SeqCst);

    if self_update_handle.join().is_err() {
        eprintln!("[LocalLauncher] Self-update monitor panicked");
        failed = true;
    }

    if failed {
        return Err("one or more local supervisors failed".into());
    }

    Ok(())
}

fn monitor_self_update(
    stop: &AtomicBool,
    state_directory: &std::path::Path,
    product: &str,
) {
    let mut observed_update_id =
        match WatcherUpdateRecord::committed_update_id(
            state_directory,
            product,
        ) {
            Ok(update_id) => update_id,

            Err(error) => {
                eprintln!(
                    "[LocalLauncher] failed to read initial self-update state: {}",
                    error
                );

                None
            }
        };

    while !stop.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(250));

        match WatcherUpdateRecord::committed_update_id(
            state_directory,
            product,
        ) {
            Ok(Some(update_id))
                if observed_update_id.as_deref()
                    != Some(update_id.as_str()) =>
            {
                eprintln!(
                    "[LocalLauncher] committed update {} detected for {}; exiting for supervisor restart",
                    update_id,
                    product
                );

                stop.store(true, Ordering::SeqCst);
                break;
            }

            Ok(Some(update_id)) => {
                observed_update_id = Some(update_id);
            }

            Ok(None) => {}

            Err(error) => {
                eprintln!(
                    "[LocalLauncher] failed to read self-update state: {}",
                    error
                );
            }
        }
    }
}

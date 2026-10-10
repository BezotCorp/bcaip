mod auth_ref;
mod dns_name;
mod extension;
mod file;
mod files;
mod health_check;
mod id;
mod launcher_cache;
mod launcher_config;
mod local_service;
mod server_config;
mod server_host;
mod server_id;
mod server_location;
mod server_port;
mod server_protocol;
mod service_config;

use files::Files;
use std::{
    env,
    error::Error,
    path::PathBuf,
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
    thread,
};

fn main() -> Result<(), Box<dyn Error>> {
    let stop = Arc::new(AtomicBool::new(false));
    let signal_stop = Arc::clone(&stop);

    ctrlc::set_handler(move || {
        signal_stop.store(true, Ordering::SeqCst);
    })?;

    let launcher_dir = match env::var_os("BCAIP_LOCAL_LAUNCHER_CONFIG_DIR") {
        Some(directory) => PathBuf::from(directory),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    };

    let files = Files::from_launcher_directory(&launcher_dir)?;
    let launcher_config = files.load_launcher_configuration()?;
    let services = launcher_config.local_services()?;

    println!("[LocalLauncher] Supervising {} service(s)", services.len());

    let handles: Vec<_> = services
        .into_iter()
        .map(|service| {
            let stop = Arc::clone(&stop);

            thread::spawn(move || {
                service.monitor(&stop);
            })
        })
        .collect();

    let mut failed = false;

    for handle in handles {
        if handle.join().is_err() {
            eprintln!("[LocalLauncher] Supervisor panicked");
            failed = true;
        }
    }

    if failed {
        return Err("One or more supervisors failed".into());
    }

    Ok(())
}

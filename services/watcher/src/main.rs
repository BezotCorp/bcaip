mod update_phase;
mod update_record;
mod update_server;
mod update_state_store;
mod watched_product;
mod watcher_request;
mod watcher_response;

use crate::update_server::{handle_connection, reconcile_pending};
use crate::update_state_store::UpdateStateStore;
use crate::watched_product::WatchedProduct;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

const DEFAULT_BIND_ADDRESS: &str = "127.0.0.1:3010";
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);

fn main() -> Result<(), Box<dyn Error>> {
    let stop = Arc::new(AtomicBool::new(false));
    let signal_stop = Arc::clone(&stop);

    ctrlc::set_handler(move || {
        signal_stop.store(true, Ordering::SeqCst);
    })?;

    let config_directory = match env::var_os("BCAIP_WATCHER_CONFIG_DIR") {
        Some(directory) => PathBuf::from(directory),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    };

    let state_directory = match env::var_os("BCAIP_WATCHER_STATE_DIR") {
        Some(directory) => PathBuf::from(directory),
        None => config_directory.join("state"),
    };

    let bind_address = env::var("BCAIP_WATCHER_BIND_ADDRESS")
        .unwrap_or_else(|_| DEFAULT_BIND_ADDRESS.to_string());

    let products = load_products(&config_directory)?;
    let state_store = UpdateStateStore::new(state_directory)?;

    for product in products.values() {
        reconcile_pending(product, &state_store)?;
    }

    let self_product = find_self_product(&products)?;

    let listener = TcpListener::bind(&bind_address)?;
    listener.set_nonblocking(true)?;

    println!(
        "[Watcher] Listening on {} for {} product(s)",
        bind_address,
        products.len()
    );

    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, peer)) => {
                stream.set_read_timeout(Some(CONNECTION_TIMEOUT))?;
                stream.set_write_timeout(Some(CONNECTION_TIMEOUT))?;

                match handle_connection(
                    stream,
                    &products,
                    &state_store,
                    self_product.as_deref(),
                ) {
                    Ok(restart_self) => {
                        if restart_self {
                            println!(
                                "[Watcher] Watcher update committed; exiting for supervisor restart"
                            );

                            stop.store(true, Ordering::SeqCst);
                        }
                    }

                    Err(error) => {
                        eprintln!(
                            "[Watcher] Connection from {} failed: {}",
                            peer,
                            error
                        );
                    }
                }
            }

            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100));
            }

            Err(error) => {
                return Err(error.into());
            }
        }
    }

    Ok(())
}

fn load_products(
    directory: &Path,
) -> Result<HashMap<String, WatchedProduct>, Box<dyn Error>> {
    let local = directory.join("watcher_products.local.ron");
    let production = directory.join("watcher_products.ron");

    let path = if local.exists() {
        local
    } else {
        production
    };

    let source = fs::read_to_string(&path)?;

    let decoded: Vec<WatchedProduct> = ron::from_str(&source)?;

    let mut products = HashMap::new();

    for product in decoded {
        product
            .validate()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

        if products.insert(product.name.clone(), product).is_some() {
            return Err(
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "duplicate watcher product",
                )
                .into(),
            );
        }
    }

    Ok(products)
}

fn find_self_product(
    products: &HashMap<String, WatchedProduct>,
) -> Result<Option<String>, Box<dyn Error>> {
    let current_executable = fs::canonicalize(env::current_exe()?)?;

    for product in products.values() {
        let path = product.executable_path();

        if !path.exists() {
            continue;
        }

        if fs::canonicalize(path)? == current_executable {
            return Ok(Some(product.name.clone()));
        }
    }

    Ok(None)
}

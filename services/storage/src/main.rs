mod data_record;
mod request_handler;
mod sqlite_engine;
mod storage_engine;
mod storage_service;
mod websocket_server;

use std::env;
use std::error::Error;
use std::path::PathBuf;

use sqlite_engine::SqliteEngine;
use storage_service::StorageService;
use websocket_server::run_server;

const DEFAULT_CACHE_MAX_ENTRIES: usize = 1_024;
const DEFAULT_CACHE_MAX_BYTES: usize = 64 * 1024 * 1024;

fn main() -> Result<(), Box<dyn Error>> {
    let path = env::var_os("BCAIP_STORAGE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("storage.sqlite3"));

    let address =
        env::var("BCAIP_STORAGE_LISTEN_ADDR").unwrap_or_else(|_| "127.0.0.1:3002".to_owned());

    let cache_max_entries =
        read_usize_env("BCAIP_STORAGE_CACHE_MAX_ENTRIES", DEFAULT_CACHE_MAX_ENTRIES)?;

    let cache_max_bytes = read_usize_env("BCAIP_STORAGE_CACHE_MAX_BYTES", DEFAULT_CACHE_MAX_BYTES)?;

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let engine = SqliteEngine::open(&path)?;

    let mut storage = StorageService::with_cache_limits(engine, cache_max_entries, cache_max_bytes);

    run_server(&address, &mut storage)
}

fn read_usize_env(name: &str, default: usize) -> Result<usize, Box<dyn Error>> {
    match env::var(name) {
        Ok(value) => Ok(value.parse()?),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(error.into()),
    }
}

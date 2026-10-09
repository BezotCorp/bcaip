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

fn main() -> Result<(), Box<dyn Error>> {
    let path = env::var_os("BCAIP_STORAGE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("storage.sqlite3"));

    let address =
        env::var("BCAIP_STORAGE_LISTEN_ADDR").unwrap_or_else(|_| "127.0.0.1:3002".to_owned());

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let engine = SqliteEngine::open(&path)?;
    let mut storage = StorageService::new(engine);

    run_server(&address, &mut storage)
}

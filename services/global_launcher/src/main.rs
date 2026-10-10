mod auth_ref;
mod dns_name;
mod id;
mod remote_server_config;
mod remote_server_location;
mod server_host;
mod server_id;
mod server_port;
mod server_protocol;

use remote_server_config::RemoteServerConfig;
use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let launcher_dir = match env::var_os("BCAIP_GLOBAL_LAUNCHER_CONFIG_DIR") {
        Some(directory) => PathBuf::from(directory),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    };

    let local_path = launcher_dir.join("launcher_servers.local.ron");
    let production_path = launcher_dir.join("launcher_servers.ron");

    let configuration_path = if local_path.exists() {
        local_path
    } else {
        production_path
    };

    let source = fs::read_to_string(&configuration_path)?;
    let servers: Vec<RemoteServerConfig> = ron::from_str(&source)?;

    RemoteServerConfig::validate_all(&servers)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    println!(
        "[GlobalLauncher] Loaded {} remote server(s)",
        servers.len()
    );

    Ok(())
}

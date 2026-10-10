use crate::health_check::HealthCheck;
use crate::server_config::ServerConfig;
use crate::server_id::ServerId;
use bincode_next::{Decode, Encode};
use serde::Deserialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Component, Path};

#[derive(Encode, Decode, Deserialize, Clone, Debug)]
pub(crate) struct ServiceConfig {
    pub(crate) name: String,
    pub(crate) server_id: ServerId,
    pub(crate) executable_path: String,
    pub(crate) health_check: HealthCheck,
}

impl ServiceConfig {
    pub(crate) fn validate_all(
        services: &[Self],
        servers: &HashMap<ServerId, &ServerConfig>,
    ) -> Result<(), String> {
        let mut names = HashSet::new();
        let mut addresses = HashSet::new();

        for service in services {
            if service.name.is_empty()
                || !service
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            {
                return Err(format!("Invalid service name: {}", service.name));
            }

            if !names.insert(service.name.as_str()) {
                return Err(format!("Duplicate service: {}", service.name));
            }

            if !servers.contains_key(&service.server_id) {
                return Err(format!(
                    "Service {} references unknown server {}",
                    service.name, service.server_id
                ));
            }

            validate_executable_path(&service.executable_path)
                .map_err(|error| format!("{} executable_path: {}", service.name, error))?;

            match service.health_check {
                HealthCheck::WebSocketPing { address } => {
                    if !addresses.insert((service.server_id.clone(), address)) {
                        return Err(format!(
                            "Duplicate WebSocket health address on server {}: {address}",
                            service.server_id
                        ));
                    }
                }
            }
        }

        Ok(())
    }
}

fn validate_executable_path(value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err("must not be empty".to_string());
    }

    let path = Path::new(value);

    if path.as_os_str().is_empty() {
        return Err("invalid executable path".to_string());
    }

    if path.is_absolute() {
        return Err("must be relative to its server root_path".to_string());
    }

    for component in path.components() {
        if matches!(
            component,
            Component::ParentDir
                | Component::RootDir
                | Component::Prefix(_)
                | Component::CurDir
        ) {
            return Err("must stay inside its server root_path".to_string());
        }
    }

    Ok(())
}

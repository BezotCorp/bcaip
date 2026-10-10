use crate::local_service::LocalService;
use crate::server_config::ServerConfig;
use crate::server_id::ServerId;
use crate::service_config::ServiceConfig;
use bincode_next::{Decode, Encode};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Encode, Decode, Deserialize, Clone, Debug)]
pub(crate) struct LauncherConfig {
    pub(crate) servers: Vec<ServerConfig>,
    pub(crate) services: Vec<ServiceConfig>,
}

impl LauncherConfig {
    pub(crate) fn validate(&self) -> Result<(), String> {
        let servers = self.server_index()?;
        ServiceConfig::validate_all(&self.services, &servers)
    }

    pub(crate) fn local_services(&self) -> Result<Vec<LocalService>, String> {
        self.validate()?;

        let servers = self.server_index()?;
        let mut local_services = Vec::new();

        for service in &self.services {
            let server = servers
                .get(&service.server_id)
                .ok_or_else(|| format!("unknown server: {}", service.server_id))?;

            let relative_executable = Path::new(&service.executable_path);
            let executable = server.location.executable_path(relative_executable);
            let root = server.location.root_path().to_path_buf();

            local_services.push(LocalService::new(service.clone(), root, executable));
        }

        Ok(local_services)
    }

    fn server_index(&self) -> Result<HashMap<ServerId, &ServerConfig>, String> {
        let mut ids = HashSet::new();
        let mut servers = HashMap::new();

        for server in &self.servers {
            server.validate()?;

            if !ids.insert(server.id.clone()) {
                return Err(format!("duplicate server: {}", server.id));
            }

            servers.insert(server.id.clone(), server);
        }

        Ok(servers)
    }
}

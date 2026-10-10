use crate::extension::Extension;
use crate::file::File;
use crate::launcher_cache::LauncherCache;
use crate::launcher_config::LauncherConfig;
use crate::server_config::ServerConfig;
use crate::service_config::ServiceConfig;
use std::error::Error;
use std::io;
use std::path::Path;

pub(crate) struct Files {
    servers: File,
    services: File,
    launcher_cache: File,
}

impl Files {
    pub(crate) fn from_launcher_directory(directory: &Path) -> io::Result<Self> {
        let local_servers = directory.join("launcher_servers.local.ron").exists();
        let local_services = directory.join("launcher_services.local.ron").exists();

        let local = match (local_servers, local_services) {
            (false, false) => false,
            (true, true) => true,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "launcher_servers.local.ron and launcher_services.local.ron must either both exist or both be absent",
                ));
            }
        };

        let suffix = if local { ".local" } else { "" };

        Ok(Self {
            servers: File::new(
                directory.to_path_buf(),
                &format!("launcher_servers{suffix}"),
                vec![Extension::Ron],
            ),
            services: File::new(
                directory.to_path_buf(),
                &format!("launcher_services{suffix}"),
                vec![Extension::Ron],
            ),
            launcher_cache: File::new(
                directory.to_path_buf(),
                &format!("launcher_config{suffix}_cache"),
                vec![Extension::Bin],
            ),
        })
    }

    pub(crate) fn load_launcher_configuration(&self) -> Result<LauncherConfig, Box<dyn Error>> {
        let config = bincode_next::config::standard();
        let servers_source = self.servers.read_to_string(Extension::Ron)?;
        let services_source = self.services.read_to_string(Extension::Ron)?;

        if let Ok(bytes) = self.launcher_cache.read_bytes(Extension::Bin) {
            if let Ok((cache, consumed)) =
                bincode_next::decode_from_slice::<LauncherCache, _>(&bytes, config)
            {
                if consumed == bytes.len()
                    && cache.matches_sources(&servers_source, &services_source)
                    && cache.configuration.validate().is_ok()
                {
                    return Ok(cache.configuration);
                }
            }
        }

        let servers: Vec<ServerConfig> = ron::from_str(&servers_source)?;
        let services: Vec<ServiceConfig> = ron::from_str(&services_source)?;
        let configuration = LauncherConfig { servers, services };

        configuration.validate()?;

        let cache = LauncherCache::new(
            servers_source,
            services_source,
            configuration.clone(),
        );
        let encoded = bincode_next::encode_to_vec(&cache, config)?;

        self.launcher_cache.write_bytes(Extension::Bin, encoded)?;

        Ok(configuration)
    }
}

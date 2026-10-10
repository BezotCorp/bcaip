use crate::launcher_config::LauncherConfig;
use bincode_next::{Decode, Encode};

const SCHEMA_VERSION: u32 = 1;

#[derive(Encode, Decode)]
pub(crate) struct LauncherCache {
    schema_version: u32,
    servers_source: String,
    services_source: String,
    pub(crate) configuration: LauncherConfig,
}

impl LauncherCache {
    pub(crate) fn new(
        servers_source: String,
        services_source: String,
        configuration: LauncherConfig,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            servers_source,
            services_source,
            configuration,
        }
    }

    pub(crate) fn matches_sources(
        &self,
        servers_source: &str,
        services_source: &str,
    ) -> bool {
        self.schema_version == SCHEMA_VERSION
            && self.servers_source == servers_source
            && self.services_source == services_source
    }
}

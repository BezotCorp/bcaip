use std::path::{Path, PathBuf};

pub(crate) struct DeploymentPaths {
    workspace_root: PathBuf,
    data_directory: PathBuf,
    servers_directory: PathBuf,
}

impl DeploymentPaths {
    pub(crate) fn discover() -> Result<Self, String> {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));

        let workspace_root = manifest
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| "unable to resolve workspace root".to_string())?
            .to_path_buf();

        let data_directory = workspace_root.join("data");
        let servers_directory = data_directory.join("servers");

        Ok(Self {
            workspace_root,
            data_directory,
            servers_directory,
        })
    }

    pub(crate) fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub(crate) fn index_ron(&self) -> PathBuf {
        self.servers_directory.join("index.ron")
    }

    pub(crate) fn index_bin(&self) -> PathBuf {
        self.servers_directory.join("index.bin")
    }

    pub(crate) fn server_ron(&self, server_key: &str) -> PathBuf {
        self.servers_directory.join(format!("{server_key}.ron"))
    }

    pub(crate) fn server_bin(&self, server_key: &str) -> PathBuf {
        self.servers_directory.join(format!("{server_key}.bin"))
    }

    pub(crate) fn executables_ron(&self) -> PathBuf {
        self.data_directory.join("executables.ron")
    }

    pub(crate) fn executables_bin(&self) -> PathBuf {
        self.data_directory.join("executables.bin")
    }
}

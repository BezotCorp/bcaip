use std::path::{Path, PathBuf};

pub(crate) struct DeploymentPaths {
    workspace_root: PathBuf,
    data_directory: PathBuf,
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

        Ok(Self {
            workspace_root,
            data_directory,
        })
    }

    pub(crate) fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub(crate) fn data_directory(&self) -> &Path {
        &self.data_directory
    }
}

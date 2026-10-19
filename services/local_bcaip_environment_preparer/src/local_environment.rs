use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

pub(crate) struct LocalEnvironment {
    executable_root_directory: PathBuf,
    data_directory: PathBuf,
    service_directory: PathBuf,
}

impl LocalEnvironment {
    pub(crate) fn discover() -> Result<Self, String> {
        if std::env::consts::OS != "linux" {
            return Err(format!(
                "local BCAIP environment preparation is not implemented for {}",
                std::env::consts::OS
            ));
        }

        let systemd = Command::new("systemctl")
            .arg("--version")
            .status()
            .map_err(|error| format!("failed to determine local service manager: {error}"))?;

        if !systemd.success() {
            return Err("this Linux environment is not managed by systemd".to_string());
        }

        Ok(Self {
            executable_root_directory: Self::required_absolute_path("BCAIP_EXECUTABLE_ROOT")?,
            data_directory: Self::required_absolute_path("BCAIP_DATA_DIR")?,
            service_directory: PathBuf::from("/etc/systemd/system"),
        })
    }

    pub(crate) fn executable_root_directory(&self) -> &Path {
        &self.executable_root_directory
    }

    pub(crate) fn data_directory(&self) -> &Path {
        &self.data_directory
    }

    pub(crate) fn service_directory(&self) -> &Path {
        &self.service_directory
    }

    fn required_absolute_path(name: &str) -> Result<PathBuf, String> {
        let value = env::var_os(name).ok_or_else(|| format!("{name} is required"))?;

        let path = PathBuf::from(value);

        if !path.is_absolute() {
            return Err(format!("{name} must be absolute: {}", path.display()));
        }

        Ok(path)
    }
}

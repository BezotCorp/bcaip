use std::{path::PathBuf, process::Command};

pub(crate) struct LocalEnvironment {
    bcaip_directory: PathBuf,
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
            bcaip_directory: PathBuf::from("/opt/bcaip"),
            data_directory: PathBuf::from("/var/lib/bcaip"),
            service_directory: PathBuf::from("/etc/systemd/system"),
        })
    }

    pub(crate) fn bcaip_directory(&self) -> &std::path::Path {
        &self.bcaip_directory
    }

    pub(crate) fn services_directory(&self) -> PathBuf {
        self.bcaip_directory.join("services")
    }

    pub(crate) fn supervisor(&self) -> PathBuf {
        self.services_directory().join("supervisor")
    }

    pub(crate) fn data_directory(&self) -> &std::path::Path {
        &self.data_directory
    }

    pub(crate) fn executables_bin(&self) -> PathBuf {
        self.data_directory.join("executables.bin")
    }

    pub(crate) fn supervisor_unit(&self) -> PathBuf {
        self.service_directory.join("bcaip-supervisor.service")
    }
}

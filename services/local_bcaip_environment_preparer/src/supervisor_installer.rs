use std::{fs, process::Command};

use crate::local_environment::LocalEnvironment;

pub(crate) struct SupervisorInstaller;

impl SupervisorInstaller {
    pub(crate) fn install(
        environment: &LocalEnvironment,
        received_supervisor: &std::path::Path,
        received_executables_bin: &std::path::Path,
    ) -> Result<(), String> {
        if !received_supervisor.is_file() {
            return Err(format!(
                "received supervisor does not exist: {}",
                received_supervisor.display()
            ));
        }

        if !received_executables_bin.is_file() {
            return Err(format!(
                "received executable data does not exist: {}",
                received_executables_bin.display()
            ));
        }

        fs::create_dir_all(environment.services_directory()).map_err(|error| {
            format!(
                "failed to create {}: {error}",
                environment.services_directory().display()
            )
        })?;

        fs::create_dir_all(environment.data_directory()).map_err(|error| {
            format!(
                "failed to create {}: {error}",
                environment.data_directory().display()
            )
        })?;

        fs::copy(received_supervisor, environment.supervisor()).map_err(|error| {
            format!(
                "failed to install supervisor to {}: {error}",
                environment.supervisor().display()
            )
        })?;

        fs::copy(received_executables_bin, environment.executables_bin()).map_err(|error| {
            format!(
                "failed to install executable data to {}: {error}",
                environment.executables_bin().display()
            )
        })?;

        let status = Command::new("chmod")
            .arg("0755")
            .arg(environment.supervisor())
            .status()
            .map_err(|error| format!("failed to execute chmod: {error}"))?;

        if !status.success() {
            return Err("failed to make supervisor executable".to_string());
        }

        Ok(())
    }
}

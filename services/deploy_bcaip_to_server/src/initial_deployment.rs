use std::path::{Path, PathBuf};

use executables_manager::{Executable, ExecutableStartPhase};
use launch_ecosystem::ExecutionEnvironment;
use servers_manager::Server;

use crate::{remote_server::RemoteServer, service_builder::ServiceBuilder};

pub(crate) struct InitialDeployment;

impl InitialDeployment {
    pub(crate) fn execute<F>(
        server: &Server,
        workspace_root: &Path,
        executables: &[Executable],
        executable_files: &[PathBuf],
        environment: ExecutionEnvironment,
        on_first_send: F,
    ) -> Result<(), String>
    where
        F: FnOnce() -> Result<(), String>,
    {
        if environment == ExecutionEnvironment::Development {
            return Err(
                "development deployment requires a local Server; remote SSH deployment is refused"
                    .to_string(),
            );
        }

        let remote = RemoteServer::for_server(server)?;
        let services = ServiceBuilder::build_for(&remote, workspace_root, executables)?;

        let transfer_directory =
            remote.create_temporary_directory(server.paths().temporary_root_path())?;

        let transferred_executables = format!("{transfer_directory}/executables");
        let transferred_definitions = format!("{transfer_directory}/definitions");

        remote.execute(&format!(
            "mkdir '{transferred_executables}' '{transferred_definitions}'"
        ))?;

        on_first_send()?;

        for executable in executables {
            let source = services.executable(executable.id()).ok_or_else(|| {
                format!(
                    "built executable is missing for {} ({})",
                    executable.name(),
                    executable.id()
                )
            })?;

            remote.send(
                source,
                &format!("{transferred_executables}/{}", executable.id()),
            )?;
        }

        for file in executable_files {
            let file_name = file
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| format!("invalid executable definition file: {}", file.display()))?;

            remote.send(file, &format!("{transferred_definitions}/{file_name}"))?;
        }

        Self::install_environment(&remote, server, environment)?;

        Self::install_definitions(&remote, server, &transferred_definitions, executable_files)?;

        Self::install_executables(&remote, server, executables, &transferred_executables)?;

        Self::start_bcaip_environment_setup(&remote, server, executables)
    }

    fn install_environment(
        remote: &RemoteServer,
        server: &Server,
        environment: ExecutionEnvironment,
    ) -> Result<(), String> {
        let data_directory = server.paths().data_root();
        let production_lock = ExecutionEnvironment::production_lock_path(&data_directory);

        remote.execute(&format!("sudo mkdir -p '{}'", data_directory.display()))?;

        match environment {
            ExecutionEnvironment::Development => {
                remote.execute(&format!("sudo rm -f '{}'", production_lock.display()))
            }

            ExecutionEnvironment::Production => remote.execute(&format!(
                "sudo sh -c ': > \"{}\"'",
                production_lock.display()
            )),
        }
    }

    fn install_definitions(
        remote: &RemoteServer,
        server: &Server,
        transferred_definitions: &str,
        executable_files: &[PathBuf],
    ) -> Result<(), String> {
        let definitions_directory = server.paths().data_root().join("executables");

        remote.execute(&format!(
            "sudo mkdir -p '{}'",
            definitions_directory.display()
        ))?;

        for file in executable_files {
            let file_name = file
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| format!("invalid executable definition file: {}", file.display()))?;

            remote.execute(&format!(
                "sudo install -m 0644                  '{transferred_definitions}/{file_name}'                  '{}/{}'",
                definitions_directory.display(),
                file_name
            ))?;
        }

        Ok(())
    }

    fn install_executables(
        remote: &RemoteServer,
        server: &Server,
        executables: &[Executable],
        transferred_executables: &str,
    ) -> Result<(), String> {
        let executable_root = server.paths().executable_root();

        for executable in executables {
            let target = executable.path(&executable_root);

            let parent = target
                .parent()
                .ok_or_else(|| format!("{} has no parent directory", target.display()))?;

            remote.execute(&format!(
                "sudo mkdir -p '{}'                  && sudo install -m 0755                  '{transferred_executables}/{}'                  '{}'",
                parent.display(),
                executable.id(),
                target.display()
            ))?;
        }

        Ok(())
    }

    fn start_bcaip_environment_setup(
        remote: &RemoteServer,
        server: &Server,
        executables: &[Executable],
    ) -> Result<(), String> {
        let executable_root = server.paths().executable_root();
        let data_root = server.paths().data_root();

        for executable in executables {
            if executable.start_phase() != ExecutableStartPhase::BcaipEnvironmentSetup {
                continue;
            }

            let working_directory = executable.working_directory_path(&executable_root);
            let executable_path = executable.path(&executable_root);

            remote.execute(&format!(
                "cd '{}' && sudo env BCAIP_EXECUTABLE_ROOT='{}' BCAIP_DATA_DIR='{}' '{}'",
                working_directory.display(),
                executable_root.display(),
                data_root.display(),
                executable_path.display()
            ))?;
        }

        Ok(())
    }
}

use std::path::{Path, PathBuf};

use executables_manager::{Executable, ExecutableStartPhase, ExecutionEnvironment};

use crate::{
    remote_server::RemoteServer, server_deployment_descriptor::ServerDeploymentDescriptor,
    service_builder::ServiceBuilder,
};

const BCAIP_DATA_DIRECTORY: &str = "/var/lib/bcaip";

pub(crate) struct InitialDeployment;

impl InitialDeployment {
    pub(crate) fn execute<F>(
        descriptor: &ServerDeploymentDescriptor,
        workspace_root: &Path,
        executables: &[Executable],
        executable_files: &[PathBuf],
        environment: ExecutionEnvironment,
        on_first_send: F,
    ) -> Result<(), String>
    where
        F: FnOnce() -> Result<(), String>,
    {
        let remote = RemoteServer::new(descriptor.ssh_target());
        let services = ServiceBuilder::build_for(&remote, workspace_root, executables)?;

        let transfer_directory = remote.create_temporary_directory()?;
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

        Self::install_environment(&remote, environment)?;
        Self::install_definitions(
            &remote,
            environment,
            &transferred_definitions,
            executable_files,
        )?;
        Self::install_executables(&remote, executables, &transferred_executables)?;

        Self::start_bcaip_environment_setup(&remote, executables)
    }

    fn install_environment(
        remote: &RemoteServer,
        environment: ExecutionEnvironment,
    ) -> Result<(), String> {
        let data_directory = Path::new(BCAIP_DATA_DIRECTORY);
        let production_lock = ExecutionEnvironment::production_lock_path(data_directory);

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
        environment: ExecutionEnvironment,
        transferred_definitions: &str,
        executable_files: &[PathBuf],
    ) -> Result<(), String> {
        let definitions_directory =
            environment.resolve(&Path::new(BCAIP_DATA_DIRECTORY).join("executables"));

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
                "sudo install -m 0644 \
                 '{transferred_definitions}/{file_name}' \
                 '{}/{}'",
                definitions_directory.display(),
                file_name
            ))?;
        }

        Ok(())
    }

    fn install_executables(
        remote: &RemoteServer,
        executables: &[Executable],
        transferred_executables: &str,
    ) -> Result<(), String> {
        for executable in executables {
            let target = executable.path();

            let parent = target
                .parent()
                .ok_or_else(|| format!("{} has no parent directory", target.display()))?;

            remote.execute(&format!(
                "sudo mkdir -p '{}' \
                 && sudo install -m 0755 \
                 '{transferred_executables}/{}' \
                 '{}'",
                parent.display(),
                executable.id(),
                target.display()
            ))?;
        }

        Ok(())
    }

    fn start_bcaip_environment_setup(
        remote: &RemoteServer,
        executables: &[Executable],
    ) -> Result<(), String> {
        for executable in executables {
            if executable.start_phase() != ExecutableStartPhase::BcaipEnvironmentSetup {
                continue;
            }

            remote.execute(&format!(
                "cd '{}' && sudo '{}'",
                executable.working_directory(),
                executable.path().display()
            ))?;
        }

        Ok(())
    }
}

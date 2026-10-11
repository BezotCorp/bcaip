use std::{net::SocketAddr, path::Path};

use crate::{
    remote_server::RemoteServer, server_deployment_descriptor::ServerDeploymentDescriptor,
    service_builder::ServiceBuilder,
};

pub(crate) struct InitialDeployment;

impl InitialDeployment {
    pub(crate) fn execute<F>(
        descriptor: &ServerDeploymentDescriptor,
        workspace_root: &Path,
        executables_bin: &Path,
        supervisor_ws_address: SocketAddr,
        on_first_send: F,
    ) -> Result<(), String>
    where
        F: FnOnce() -> Result<(), String>,
    {
        let remote = RemoteServer::new(descriptor.ssh_target());

        let remote_os = remote.operating_system()?;
        let remote_architecture = remote.architecture()?;

        let services = ServiceBuilder::build(workspace_root, &remote_os, &remote_architecture)?;

        let directory = remote.create_temporary_directory()?;

        let remote_supervisor = format!("{directory}/supervisor");
        let remote_preparer = format!("{directory}/local_bcaip_environment_preparer");
        let remote_executables = format!("{directory}/executables.bin");

        on_first_send()?;

        remote.send(services.preparer(), &remote_preparer)?;
        remote.send(services.supervisor(), &remote_supervisor)?;
        remote.send(executables_bin, &remote_executables)?;

        remote.execute(&format!(
            "chmod 0755 '{remote_supervisor}' '{remote_preparer}' \
             && sudo '{remote_preparer}' \
             --supervisor '{remote_supervisor}' \
             --executables-bin '{remote_executables}' \
             --supervisor-port {}",
            supervisor_ws_address.port(),
        ))
    }
}

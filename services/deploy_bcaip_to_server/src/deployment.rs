use executables_manager::{ExecutableRole, ExecutableStartPhase, ExecutablesManager};
use launch_ecosystem::ExecutionEnvironment;
use servers_manager::{Server, ServerDeploymentStatus, ServersManager};

use crate::{
    deployment_paths::DeploymentPaths, initial_deployment::InitialDeployment,
    supervisor_health::SupervisorHealth,
};

pub(crate) struct Deployment {
    paths: DeploymentPaths,
    environment: ExecutionEnvironment,
}

impl Deployment {
    pub(crate) fn new(environment: ExecutionEnvironment) -> Result<Self, String> {
        Ok(Self {
            paths: DeploymentPaths::discover()?,
            environment,
        })
    }

    pub(crate) fn run(&self) -> Result<(), String> {
        let servers_manager = ServersManager::new(self.paths.data_directory(), self.environment);

        servers_manager.ensure_materialized()?;

        let server_index = servers_manager.index_by_name()?;

        if !server_index
            .entries()
            .any(|(_, entry)| entry.is_deployment_target())
        {
            println!("[DeployBcaipToServer] no server requires deployment");
            return Ok(());
        }

        let executables_manager =
            ExecutablesManager::new(self.paths.data_directory(), self.environment);

        executables_manager.ensure_materialized()?;

        let executables = executables_manager.list()?;
        let executable_files = executables_manager.binary_files()?;

        if !executables.iter().any(|executable| {
            executable.start_phase() == ExecutableStartPhase::BcaipEnvironmentSetup
        }) {
            return Err("no executable belongs to BcaipEnvironmentSetup".to_string());
        }

        let authority =
            executables_manager.resolve_unique_by_role(ExecutableRole::BcaipAuthorityServer)?;

        let supervisor_ws_address = authority.default_ws_address();
        let supervisor_name = authority.name().as_str();

        for (_, entry) in server_index.entries() {
            if !entry.is_deployment_target() {
                continue;
            }

            let server = servers_manager.resolve_by_id(entry.id())?;

            match entry.deployment_status() {
                ServerDeploymentStatus::NeverBooted => {
                    InitialDeployment::execute(
                        &server,
                        self.paths.workspace_root(),
                        &executables,
                        &executable_files,
                        self.environment,
                        || {
                            servers_manager
                                .set_deployment_status(server.id(), ServerDeploymentStatus::Pending)
                        },
                    )?;

                    self.confirm_boot(
                        &servers_manager,
                        &server,
                        supervisor_ws_address,
                        supervisor_name,
                    )?;
                }

                ServerDeploymentStatus::Pending => {
                    self.confirm_boot(
                        &servers_manager,
                        &server,
                        supervisor_ws_address,
                        supervisor_name,
                    )?;
                }

                ServerDeploymentStatus::Booted => {}
            }
        }

        Ok(())
    }

    fn confirm_boot(
        &self,
        servers_manager: &ServersManager,
        server: &Server,
        supervisor_ws_address: std::net::SocketAddr,
        supervisor_name: &str,
    ) -> Result<(), String> {
        if SupervisorHealth::check(server, supervisor_ws_address, supervisor_name)? {
            servers_manager.set_deployment_status(server.id(), ServerDeploymentStatus::Booted)?;

            println!("[DeployBcaipToServer] {} is booted", server.name());
        } else {
            println!("[DeployBcaipToServer] {} is pending", server.name());
        }

        Ok(())
    }
}

use executables_manager::{
    ExecutableRole, ExecutableStartPhase, ExecutablesManager, ExecutionEnvironment,
};

use crate::{
    deployment_paths::DeploymentPaths, initial_deployment::InitialDeployment,
    server_deployment_descriptor::ServerDeploymentDescriptor,
    server_deployment_index::ServerDeploymentIndex,
    server_deployment_status::ServerDeploymentStatus, supervisor_health::SupervisorHealth,
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
        let mut index =
            ServerDeploymentIndex::load(self.paths.index_ron(), self.paths.index_bin())?;

        let has_deployment = (0..index.len()).any(|position| {
            index
                .entry(position)
                .is_some_and(|entry| entry.should_deploy())
        });

        if !has_deployment {
            println!("[DeployBcaipToServer] no server requires deployment");
            return Ok(());
        }

        // This side owns the RON sources: the shipped `.bin` are materialized from them.
        let manager = ExecutablesManager::new(self.paths.data_directory(), self.environment);

        manager.ensure_materialized()?;

        let executables = manager.list()?;
        let executable_files = manager.binary_files()?;

        if !executables.iter().any(|executable| {
            executable.start_phase() == ExecutableStartPhase::BcaipEnvironmentSetup
        }) {
            return Err("no executable belongs to BcaipEnvironmentSetup".to_string());
        }

        let authority = manager.resolve_unique_by_role(ExecutableRole::BcaipAuthorityServer)?;

        let supervisor_ws_address = authority.default_ws_address();
        let supervisor_name = authority.name().as_str();

        for position in 0..index.len() {
            let Some(entry) = index.entry(position) else {
                continue;
            };

            if !entry.should_deploy() {
                continue;
            }

            let server_key = entry.server_key().to_string();
            let status = entry.status();

            let descriptor = ServerDeploymentDescriptor::materialize_and_load(
                self.paths.server_ron(&server_key),
                self.paths.server_bin(&server_key),
            )?;

            match status {
                ServerDeploymentStatus::NeverBooted => {
                    InitialDeployment::execute(
                        &descriptor,
                        self.paths.workspace_root(),
                        &executables,
                        &executable_files,
                        self.environment,
                        || {
                            let entry = index.entry_mut(position).ok_or_else(|| {
                                format!("server index entry disappeared: {server_key}")
                            })?;

                            entry.mark_pending();
                            index.save()
                        },
                    )?;

                    if SupervisorHealth::check(&descriptor, supervisor_ws_address, supervisor_name)?
                    {
                        let entry = index.entry_mut(position).ok_or_else(|| {
                            format!("server index entry disappeared: {server_key}")
                        })?;

                        entry.mark_booted();
                        index.save()?;

                        println!("[DeployBcaipToServer] {server_key} is booted");
                    } else {
                        println!("[DeployBcaipToServer] {server_key} is pending");
                    }
                }

                ServerDeploymentStatus::Pending => {
                    if SupervisorHealth::check(&descriptor, supervisor_ws_address, supervisor_name)?
                    {
                        let entry = index.entry_mut(position).ok_or_else(|| {
                            format!("server index entry disappeared: {server_key}")
                        })?;

                        entry.mark_booted();
                        index.save()?;

                        println!("[DeployBcaipToServer] {server_key} is booted");
                    } else {
                        println!("[DeployBcaipToServer] {server_key} is still pending");
                    }
                }

                ServerDeploymentStatus::Booted => {}
            }
        }

        Ok(())
    }
}

use crate::{
    deployment_paths::DeploymentPaths, executable_data::ExecutableData,
    initial_deployment::InitialDeployment,
    server_deployment_descriptor::ServerDeploymentDescriptor,
    server_deployment_index::ServerDeploymentIndex,
    server_deployment_status::ServerDeploymentStatus, supervisor_health::SupervisorHealth,
};

pub(crate) struct Deployment {
    paths: DeploymentPaths,
}

impl Deployment {
    pub(crate) fn new() -> Result<Self, String> {
        Ok(Self {
            paths: DeploymentPaths::discover()?,
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

        let executable_data = ExecutableData::load_or_materialize(
            self.paths.executables_ron(),
            self.paths.executables_bin(),
        )?;

        let supervisor_ws_address = executable_data.supervisor()?.default_ws_address();

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
                        executable_data.binary_path(),
                        supervisor_ws_address,
                        || {
                            let entry = index.entry_mut(position).ok_or_else(|| {
                                format!("server index entry disappeared: {server_key}")
                            })?;

                            entry.mark_pending();
                            index.save()
                        },
                    )?;

                    if SupervisorHealth::check(&descriptor, supervisor_ws_address)? {
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
                    if SupervisorHealth::check(&descriptor, supervisor_ws_address)? {
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

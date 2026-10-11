use crate::{
    local_environment::LocalEnvironment, preparer_arguments::PreparerArguments,
    supervisor_installer::SupervisorInstaller, systemd_integration::SystemdIntegration,
};

pub(crate) struct LocalBcaipEnvironmentPreparer;

impl LocalBcaipEnvironmentPreparer {
    pub(crate) fn run() -> Result<(), String> {
        let arguments = PreparerArguments::parse()?;
        let environment = LocalEnvironment::discover()?;

        println!(
            "[LocalBcaipEnvironmentPreparer] preparing BCAIP in {}",
            environment.bcaip_directory().display()
        );

        SupervisorInstaller::install(
            &environment,
            arguments.supervisor(),
            arguments.executables_bin(),
        )?;

        SystemdIntegration::install(&environment, arguments.supervisor_port())?;

        println!("[LocalBcaipEnvironmentPreparer] local BCAIP environment is prepared");

        Ok(())
    }
}

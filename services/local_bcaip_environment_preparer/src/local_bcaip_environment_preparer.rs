use crate::{local_environment::LocalEnvironment, systemd_integration::SystemdIntegration};

pub(crate) struct LocalBcaipEnvironmentPreparer;

impl LocalBcaipEnvironmentPreparer {
    pub(crate) fn run() -> Result<(), String> {
        let environment = LocalEnvironment::discover()?;

        println!(
            "[LocalBcaipEnvironmentPreparer] preparing BCAIP in {}",
            environment.bcaip_directory().display()
        );

        SystemdIntegration::install(&environment)?;

        println!("[LocalBcaipEnvironmentPreparer] local BCAIP environment is prepared");

        Ok(())
    }
}

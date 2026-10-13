mod file_writer;
mod local_bcaip_environment_preparer;
mod local_environment;
mod systemd_integration;

use local_bcaip_environment_preparer::LocalBcaipEnvironmentPreparer;

fn main() -> Result<(), String> {
    LocalBcaipEnvironmentPreparer::run()
}

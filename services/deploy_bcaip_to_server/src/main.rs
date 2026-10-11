mod built_services;
mod deployment;
mod deployment_paths;
mod executable;
mod executable_data;
mod initial_deployment;
mod process;
mod remote_server;
mod server_deployment_descriptor;
mod server_deployment_index;
mod server_deployment_index_entry;
mod server_deployment_status;
mod service_builder;
mod supervisor_health;
mod supervisor_health_request;
mod supervisor_health_response;
mod update;
mod update_phase;

use deployment::Deployment;

fn main() -> Result<(), String> {
    Deployment::new()?.run()
}

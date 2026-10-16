mod built_services;
mod deployment;
mod deployment_paths;
mod initial_deployment;
mod remote_server;
mod service_builder;
mod supervisor_health;
mod supervisor_health_request;
mod supervisor_health_response;

use deployment::Deployment;
use launch_ecosystem::ExecutionEnvironment;

fn main() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);

    if arguments.next().as_deref() != Some("--environment") {
        return Err("--environment development|production is required".to_string());
    }

    let environment = match arguments
        .next()
        .ok_or_else(|| "--environment requires development or production".to_string())?
        .as_str()
    {
        "development" => ExecutionEnvironment::Development,
        "production" => ExecutionEnvironment::Production,
        value => {
            return Err(format!(
                "invalid environment {value}: expected development or production"
            ));
        }
    };

    if let Some(argument) = arguments.next() {
        return Err(format!("unexpected argument: {argument}"));
    }

    Deployment::new(environment)?.run()
}

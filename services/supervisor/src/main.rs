mod atomic_file;
mod authority_state;
mod auto_update_diagnostic;
mod auto_update_failure;
mod auto_update_phase;
mod auto_updater;
mod begin_update_result;
mod candidate_client;
mod candidate_process;
mod candidate_report;
mod executable;
mod executable_backup;
mod executable_launcher;
mod executable_management_request;
mod executable_management_response;
mod executable_runtime;
mod executable_store;
mod handover_journal;
mod handover_phase;
mod handover_recovery;
mod legacy_executable_state;
mod managed_process;
mod monitoring_state;
mod runtime_handover;
mod runtime_state_backup;
mod supervisor;
mod supervisor_config;
mod supervisor_monitor;
mod systemd_notifier;
mod update_server;
mod ws_health;

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    supervisor::Supervisor::from_env()?.run()
}

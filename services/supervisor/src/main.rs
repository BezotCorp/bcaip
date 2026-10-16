mod atomic_file;
mod auto_update_diagnostic;
mod auto_update_failure;
mod auto_update_phase;
mod auto_updater;
mod candidate_client;
mod candidate_process;
mod executable_backup;
mod executable_launcher;
mod executable_store;
mod handover_journal;
mod handover_phase;
mod handover_recovery;
mod managed_process;
mod monitoring_state;
mod runtime_handover;
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

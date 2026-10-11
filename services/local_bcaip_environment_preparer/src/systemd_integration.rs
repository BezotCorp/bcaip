use std::process::Command;

use crate::{file_writer::FileWriter, local_environment::LocalEnvironment};

pub(crate) struct SystemdIntegration;

impl SystemdIntegration {
    pub(crate) fn install(
        environment: &LocalEnvironment,
        supervisor_port: u16,
    ) -> Result<(), String> {
        let unit = format!(
            r#"[Unit]
Description=BCAIP supervisor
After=network.target

[Service]
Type=notify
NotifyAccess=all
WorkingDirectory={}
ExecStart={}
Restart=always
RestartSec=1
KillMode=control-group
TimeoutStopSec=15
Environment=BCAIP_SUPERVISOR_NAME=supervisor
Environment=BCAIP_DATA_DIR={}
Environment=BCAIP_SUPERVISOR_BIND_ADDRESS=127.0.0.1:{}

[Install]
WantedBy=multi-user.target
"#,
            environment.bcaip_directory().display(),
            environment.supervisor().display(),
            environment.data_directory().display(),
            supervisor_port,
        );

        FileWriter::write(&environment.supervisor_unit(), unit.as_bytes())?;

        Self::systemctl(&["daemon-reload"])?;

        Self::systemctl(&["enable", "--now", "bcaip-supervisor.service"])?;

        Self::systemctl(&["is-active", "--quiet", "bcaip-supervisor.service"])
    }

    fn systemctl(arguments: &[&str]) -> Result<(), String> {
        let status = Command::new("systemctl")
            .args(arguments)
            .status()
            .map_err(|error| {
                format!(
                    "failed to execute systemctl {}: {error}",
                    arguments.join(" ")
                )
            })?;

        if !status.success() {
            return Err(format!("systemctl {} failed", arguments.join(" ")));
        }

        Ok(())
    }
}

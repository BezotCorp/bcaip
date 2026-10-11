use crate::executable::Executable;
use std::net::SocketAddr;
use std::process::Command;

pub(crate) struct Systemd;

impl Systemd {
    pub(crate) fn restart_transient(
        executable: &Executable,
        instance_id: &str,
        supervisor_address: SocketAddr,
    ) -> Result<(), String> {
        let unit = Self::transient_unit_name(executable.name());

        let _ = Command::new("systemctl").arg("stop").arg(&unit).output();

        let _ = Command::new("systemctl")
            .arg("reset-failed")
            .arg(&unit)
            .output();

        let path = executable.path();
        let applied_update_id = executable.committed_update_id();
        let mut command = Command::new("systemd-run");

        command
            .arg("--quiet")
            .arg("--collect")
            .arg(format!("--unit={unit}"))
            .arg("--slice=bcaip.slice")
            .arg("--property=Type=simple")
            .arg("--property=Restart=no")
            .arg(format!(
                "--property=WorkingDirectory={}",
                executable.working_directory()
            ))
            .arg(format!(
                "--setenv=BCAIP_EXECUTABLE_NAME={}",
                executable.name()
            ))
            .arg(format!("--setenv=BCAIP_INSTANCE_ID={}", instance_id))
            .arg(format!(
                "--setenv=BCAIP_WS_ADDRESS={}",
                executable.default_ws_address()
            ))
            .arg(format!(
                "--setenv=BCAIP_SUPERVISOR_ADDRESS={}",
                supervisor_address
            ));

        if let Some(update_id) = applied_update_id {
            command.arg(format!("--setenv=BCAIP_APPLIED_UPDATE_ID={}", update_id));
        }

        let output = command.arg(&path).output().map_err(|error| {
            format!(
                "failed to execute systemd-run for {}: {}",
                executable.name(),
                error
            )
        })?;

        if !output.status.success() {
            return Err(format!(
                "systemd-run failed for {}: {}",
                executable.name(),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        Ok(())
    }

    fn transient_unit_name(executable_name: &str) -> String {
        format!("bcaip-executable-{}.service", executable_name)
    }
}

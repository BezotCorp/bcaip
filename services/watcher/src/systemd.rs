use crate::executable::Executable;
use crate::update_phase::UpdatePhase;
use std::net::SocketAddr;
use std::process::Command;

const LAUNCHER_UNIT: &str =
    "bcaip-local-launcher.service";

const WATCHER_UNIT: &str =
    "bcaip-watcher.service";

pub(crate) fn restart_persistent(
    executable_name: &str,
    launcher_name: &str,
    watcher_name: &str,
) -> Result<(), String> {
    let unit =
        if executable_name == launcher_name {
            LAUNCHER_UNIT
        } else if executable_name == watcher_name {
            WATCHER_UNIT
        } else {
            return Err(format!(
                "{} is not a persistent executable",
                executable_name
            ));
        };

    restart_persistent_unit(
        unit,
    )
}

pub(crate) fn restart_self() -> Result<(), String> {
    let output = Command::new("systemctl")
        .arg("--no-block")
        .arg("restart")
        .arg(WATCHER_UNIT)
        .output()
        .map_err(|error| {
            format!(
                "failed to execute watcher self restart: {}",
                error
            )
        })?;

    if !output.status.success() {
        return Err(format!(
            "watcher self restart failed: {}",
            String::from_utf8_lossy(
                &output.stderr
            )
            .trim()
        ));
    }

    Ok(())
}

fn restart_persistent_unit(
    unit: &str,
) -> Result<(), String> {
    let output = Command::new("systemctl")
        .arg("restart")
        .arg(unit)
        .output()
        .map_err(|error| {
            format!(
                "failed to execute systemctl restart {}: {}",
                unit,
                error
            )
        })?;

    if !output.status.success() {
        return Err(format!(
            "systemctl restart {} failed: {}",
            unit,
            String::from_utf8_lossy(
                &output.stderr
            )
            .trim()
        ));
    }

    Ok(())
}

pub(crate) fn restart_transient(
    executable: &Executable,
    instance_id: &str,
    watcher_address: SocketAddr,
) -> Result<(), String> {
    let unit = transient_unit_name(
        &executable.name,
    );

    let _ = Command::new("systemctl")
        .arg("stop")
        .arg(&unit)
        .output();

    let _ = Command::new("systemctl")
        .arg("reset-failed")
        .arg(&unit)
        .output();

    let path =
        executable.path();

    let applied_update_id =
        executable
            .update
            .as_ref()
            .filter(|update| {
                update.phase
                    == UpdatePhase::Committed
            })
            .map(|update| {
                update.update_id.as_str()
            });

    let mut command =
        Command::new("systemd-run");

    command
            .arg("--quiet")
            .arg("--collect")
            .arg(format!(
                "--unit={unit}"
            ))
            .arg("--slice=bcaip.slice")
            .arg("--property=Type=simple")
            .arg("--property=Restart=no")
            .arg(format!(
                "--property=WorkingDirectory={}",
                executable.working_directory
            ))
            .arg(format!(
                "--setenv=BCAIP_EXECUTABLE_NAME={}",
                executable.name
            ))
            .arg(format!(
                "--setenv=BCAIP_INSTANCE_ID={}",
                instance_id
            ))
            .arg(format!(
                "--setenv=BCAIP_WS_ADDRESS={}",
                executable.default_ws_address
            ))
            .arg(format!(
                "--setenv=BCAIP_WATCHER_ADDRESS={}",
                watcher_address
            ));

    if let Some(update_id) =
        applied_update_id
    {
        command.arg(format!(
            "--setenv=BCAIP_APPLIED_UPDATE_ID={}",
            update_id
        ));
    }

    let output =
        command
            .arg(&path)
            .output()
            .map_err(|error| {
                format!(
                    "failed to execute systemd-run for {}: {}",
                    executable.name,
                    error
                )
            })?;

    if !output.status.success() {
        return Err(format!(
            "systemd-run failed for {}: {}",
            executable.name,
            String::from_utf8_lossy(
                &output.stderr
            )
            .trim()
        ));
    }

    Ok(())
}

fn transient_unit_name(
    executable_name: &str,
) -> String {
    format!(
        "bcaip-executable-{}.service",
        executable_name
    )
}

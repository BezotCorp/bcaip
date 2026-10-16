use std::{collections::HashSet, fs, process::Command};

use executables_manager::{
    Executable, ExecutableStartPhase, ExecutablesManager, ExecutionEnvironment,
};

use crate::{file_writer::FileWriter, local_environment::LocalEnvironment};

const UNIT_PREFIX: &str = "bcaip-";
const UNIT_SUFFIX: &str = ".service";
// Owned by the supervisor (its handover journal); only its presence matters here.
const HANDOVER_JOURNAL: &str = "supervisor-handover.ron";
const OWNER_MARKER: &str = "GeneratedBy=local_bcaip_environment_preparer";

pub(crate) struct SystemdIntegration;

impl SystemdIntegration {
    pub(crate) fn install(environment: &LocalEnvironment) -> Result<(), String> {
        let manager = ExecutablesManager::new(
            environment.data_directory(),
            ExecutionEnvironment::detect(environment.data_directory())?,
        );

        let executables =
            manager.list_by_start_phase(ExecutableStartPhase::AfterBcaipEnvironmentSetup)?;

        let stale_units = Self::stale_units(environment, &executables)?;

        if !stale_units.is_empty() {
            Self::ensure_no_handover(environment)?;
        }

        for executable in &executables {
            Self::install_after_bcaip_environment_setup_executable(environment, executable)?;
        }

        Self::systemctl(&["daemon-reload"])?;

        // The former units are only disabled while they still run, so they do
        // not come back at boot; they are stopped right before the units that
        // replace them are started (both cannot be the supervisor at once),
        // and their files are removed once the replacements are active.
        for unit in &stale_units {
            Self::systemctl(&["disable", unit])?;
        }

        for unit in &stale_units {
            Self::systemctl(&["stop", unit])?;
        }

        for executable in &executables {
            let unit = Self::unit_name(executable);

            Self::systemctl(&["enable", "--now", &unit])?;

            Self::systemctl(&["is-active", "--quiet", &unit])?;
        }

        for unit in &stale_units {
            let path = environment.service_directory().join(unit);

            fs::remove_file(&path)
                .map_err(|error| format!("failed to remove {}: {error}", path.display()))?;
        }

        if !stale_units.is_empty() {
            Self::systemctl(&["daemon-reload"])?;
        }

        Ok(())
    }

    /// A handover journal means an auto-update is in progress (or was
    /// interrupted and awaits the recovery of the next start). The supervisor
    /// that carries it must not be stopped from under it.
    fn ensure_no_handover(environment: &LocalEnvironment) -> Result<(), String> {
        let journal = environment.data_directory().join(HANDOVER_JOURNAL);

        if journal.exists() {
            return Err(format!(
                "a supervisor handover is in progress ({} exists); retry once it is finished",
                journal.display()
            ));
        }

        Ok(())
    }

    /// The unit is `Type=notify` with `NotifyAccess=all`: an AfterBcaipEnvironmentSetup
    /// executable reports readiness to systemd, and may hand over the unit's
    /// main process from another process of the unit (the supervisor does).
    fn install_after_bcaip_environment_setup_executable(
        environment: &LocalEnvironment,
        executable: &Executable,
    ) -> Result<(), String> {
        let working_directory =
            executable.working_directory_path(environment.executable_root_directory());
        let executable_path = executable.path(environment.executable_root_directory());

        let unit = format!(
            r#"[Unit]
Description=BCAIP {}
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
Environment=BCAIP_EXECUTABLE_ID={}
Environment=BCAIP_EXECUTABLE_ROOT={}
Environment=BCAIP_DATA_DIR={}

[Install]
WantedBy=multi-user.target

[X-BCAIP]
{}
ExecutableId={}
"#,
            executable.name(),
            working_directory.display(),
            executable_path.display(),
            executable.id(),
            environment.executable_root_directory().display(),
            environment.data_directory().display(),
            OWNER_MARKER,
            executable.id(),
        );

        FileWriter::write(
            &environment
                .service_directory()
                .join(Self::unit_name(executable)),
            unit.as_bytes(),
        )
    }

    /// Units are named after the stable `ExecutableId`. A unit left in the
    /// service directory that this preparer generated (current marker, or the
    /// former name-based layout, recognised by its generated content for this
    /// data directory) but that is no longer expected is stale. Any other
    /// unit is not ours and is left alone.
    fn stale_units(
        environment: &LocalEnvironment,
        executables: &[Executable],
    ) -> Result<Vec<String>, String> {
        let expected: HashSet<String> = executables.iter().map(Self::unit_name).collect();
        let mut stale = Vec::new();

        let entries = fs::read_dir(environment.service_directory()).map_err(|error| {
            format!(
                "failed to read {}: {error}",
                environment.service_directory().display()
            )
        })?;

        for entry in entries {
            let entry = entry.map_err(|error| {
                format!(
                    "failed to read entry in {}: {error}",
                    environment.service_directory().display()
                )
            })?;

            let Some(unit) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };

            if !unit.starts_with(UNIT_PREFIX)
                || !unit.ends_with(UNIT_SUFFIX)
                || expected.contains(&unit)
            {
                continue;
            }

            let Ok(content) = fs::read_to_string(entry.path()) else {
                continue;
            };

            if Self::is_generated_by_preparer(environment, &content) {
                println!("[LocalBcaipEnvironmentPreparer] stale unit {unit} will be replaced");

                stale.push(unit);
            } else {
                println!(
                    "[LocalBcaipEnvironmentPreparer] leaving {unit}: not generated by the preparer"
                );
            }
        }

        Ok(stale)
    }

    fn is_generated_by_preparer(environment: &LocalEnvironment, content: &str) -> bool {
        if content.lines().any(|line| line == OWNER_MARKER) {
            return true;
        }

        // Former layout, written before the marker existed: it set both of
        // these environment lines, the second one for this very data directory.
        let legacy_data_directory = format!(
            "Environment=BCAIP_DATA_DIR={}",
            environment.data_directory().display()
        );

        content
            .lines()
            .any(|line| line.starts_with("Environment=BCAIP_SUPERVISOR_NAME="))
            && content.lines().any(|line| line == legacy_data_directory)
    }

    fn unit_name(executable: &Executable) -> String {
        format!("{UNIT_PREFIX}{}{UNIT_SUFFIX}", executable.id())
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

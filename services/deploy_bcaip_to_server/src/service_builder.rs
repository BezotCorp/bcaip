use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use executables_manager::Executable;
use serde_json::Value;

use crate::{built_services::BuiltServices, remote_server::RemoteServer};

pub(crate) struct ServiceBuilder;

impl ServiceBuilder {
    pub(crate) fn build_for(
        remote: &RemoteServer,
        workspace_root: &Path,
        executables: &[Executable],
    ) -> Result<BuiltServices, String> {
        let remote_os = remote.operating_system()?;
        let remote_architecture = remote.architecture()?;
        let target = Self::rust_target(&remote_os, &remote_architecture)?;

        let mut binaries = Vec::new();

        for executable in executables {
            let binary = Self::binary_name(executable)?;

            if !binaries.contains(&binary) {
                binaries.push(binary);
            }
        }

        let mut command = Command::new("cargo");

        command
            .current_dir(workspace_root)
            .args(["build", "--release", "--target", target]);

        for binary in &binaries {
            command.args(["--bin", binary]);
        }

        let output = command
            .arg("--message-format=json-render-diagnostics")
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .output()
            .map_err(|error| format!("failed to execute cargo build: {error}"))?;

        if !output.status.success() {
            return Err(format!("cargo build failed for Rust target {target}"));
        }

        let stdout = String::from_utf8(output.stdout)
            .map_err(|error| format!("cargo build returned invalid UTF-8: {error}"))?;

        let mut built: HashMap<String, PathBuf> = HashMap::new();

        for line in stdout.lines() {
            let Ok(message) = serde_json::from_str::<Value>(line) else {
                continue;
            };

            if message.get("reason").and_then(Value::as_str) != Some("compiler-artifact") {
                continue;
            }

            let Some(name) = message
                .get("target")
                .and_then(|target| target.get("name"))
                .and_then(Value::as_str)
            else {
                continue;
            };

            let Some(executable) = message.get("executable").and_then(Value::as_str) else {
                continue;
            };

            built.insert(name.to_string(), PathBuf::from(executable));
        }

        let mut built_executables = HashMap::new();

        for executable in executables {
            let binary = Self::binary_name(executable)?;
            let path = built
                .get(&binary)
                .ok_or_else(|| format!("cargo did not return the binary {binary}"))?
                .clone();

            built_executables.insert(executable.id(), path);
        }

        Ok(BuiltServices::new(built_executables))
    }

    fn binary_name(executable: &Executable) -> Result<String, String> {
        Path::new(executable.executable_path())
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_string)
            .ok_or_else(|| format!("{} has no binary file name", executable.name()))
    }

    fn rust_target(remote_os: &str, remote_architecture: &str) -> Result<&'static str, String> {
        match (remote_os, remote_architecture) {
            ("Linux", "x86_64") => Ok("x86_64-unknown-linux-gnu"),
            ("Linux", "aarch64" | "arm64") => Ok("aarch64-unknown-linux-gnu"),
            _ => Err(format!(
                "no Rust build target is defined yet for {remote_os}/{remote_architecture}"
            )),
        }
    }
}

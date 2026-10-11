use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde_json::Value;

use crate::built_services::BuiltServices;

pub(crate) struct ServiceBuilder;

impl ServiceBuilder {
    pub(crate) fn build(
        workspace_root: &Path,
        remote_os: &str,
        remote_architecture: &str,
    ) -> Result<BuiltServices, String> {
        let target = Self::rust_target(remote_os, remote_architecture)?;

        let output = Command::new("cargo")
            .current_dir(workspace_root)
            .args([
                "build",
                "--release",
                "--target",
                target,
                "-p",
                "supervisor",
                "-p",
                "local_bcaip_environment_preparer",
                "--message-format=json-render-diagnostics",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .output()
            .map_err(|error| format!("failed to execute cargo build: {error}"))?;

        if !output.status.success() {
            return Err(format!("cargo build failed for Rust target {target}"));
        }

        let stdout = String::from_utf8(output.stdout)
            .map_err(|error| format!("cargo build returned invalid UTF-8: {error}"))?;

        let mut supervisor = None;
        let mut preparer = None;

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

            match name {
                "supervisor" => supervisor = Some(PathBuf::from(executable)),
                "local_bcaip_environment_preparer" => preparer = Some(PathBuf::from(executable)),
                _ => {}
            }
        }

        Ok(BuiltServices::new(
            supervisor.ok_or_else(|| "cargo did not return the supervisor binary".to_string())?,
            preparer.ok_or_else(|| {
                "cargo did not return the local_bcaip_environment_preparer binary".to_string()
            })?,
        ))
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

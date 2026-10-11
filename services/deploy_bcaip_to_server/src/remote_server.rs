use std::{
    path::Path,
    process::{Command, Output},
};

pub(crate) struct RemoteServer {
    ssh_target: String,
}

impl RemoteServer {
    pub(crate) fn new(ssh_target: &str) -> Self {
        Self {
            ssh_target: ssh_target.to_string(),
        }
    }

    pub(crate) fn operating_system(&self) -> Result<String, String> {
        self.read_command("uname -s")
    }

    pub(crate) fn architecture(&self) -> Result<String, String> {
        self.read_command("uname -m")
    }

    pub(crate) fn create_temporary_directory(&self) -> Result<String, String> {
        let directory = self.read_command("mktemp -d /tmp/bcaip.XXXXXXXX")?;

        if !directory.starts_with("/tmp/bcaip.") {
            return Err(format!(
                "remote server returned unexpected temporary directory: {directory}"
            ));
        }

        Ok(directory)
    }

    pub(crate) fn send(&self, source: &Path, remote_path: &str) -> Result<(), String> {
        let destination = format!("{}:{remote_path}", self.ssh_target);

        let output = Command::new("scp")
            .arg("-q")
            .arg("-o")
            .arg("BatchMode=yes")
            .arg(source)
            .arg(destination)
            .output()
            .map_err(|error| format!("failed to execute scp: {error}"))?;

        if !output.status.success() {
            return Err(format!(
                "scp failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        Ok(())
    }

    pub(crate) fn execute(&self, command: &str) -> Result<(), String> {
        let output = self.run(command)?;

        if output.status.success() {
            return Ok(());
        }

        Err(format!(
            "remote command failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }

    fn read_command(&self, command: &str) -> Result<String, String> {
        let output = self.run(command)?;

        if !output.status.success() {
            return Err(format!(
                "remote command failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        Ok(String::from_utf8(output.stdout)
            .map_err(|error| format!("remote command returned invalid UTF-8: {error}"))?
            .trim()
            .to_string())
    }

    fn run(&self, command: &str) -> Result<Output, String> {
        Command::new("ssh")
            .arg("-o")
            .arg("BatchMode=yes")
            .arg(&self.ssh_target)
            .arg(command)
            .output()
            .map_err(|error| format!("failed to execute ssh: {error}"))
    }
}

use std::path::PathBuf;

pub(crate) struct PreparerArguments {
    supervisor: PathBuf,
    executables_bin: PathBuf,
    supervisor_port: u16,
}

impl PreparerArguments {
    pub(crate) fn parse() -> Result<Self, String> {
        let mut arguments = std::env::args().skip(1);

        let mut supervisor = None;
        let mut executables_bin = None;
        let mut supervisor_port = None;

        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--supervisor" => {
                    supervisor = Some(PathBuf::from(
                        arguments
                            .next()
                            .ok_or_else(|| "--supervisor requires a path".to_string())?,
                    ));
                }

                "--executables-bin" => {
                    executables_bin =
                        Some(PathBuf::from(arguments.next().ok_or_else(|| {
                            "--executables-bin requires a path".to_string()
                        })?));
                }

                "--supervisor-port" => {
                    let value = arguments
                        .next()
                        .ok_or_else(|| "--supervisor-port requires a value".to_string())?;

                    let port = value
                        .parse::<u16>()
                        .map_err(|error| format!("invalid supervisor port: {error}"))?;

                    if port == 0 {
                        return Err("supervisor port must not be zero".to_string());
                    }

                    supervisor_port = Some(port);
                }

                other => return Err(format!("unknown argument: {other}")),
            }
        }

        Ok(Self {
            supervisor: supervisor.ok_or_else(|| "--supervisor is required".to_string())?,
            executables_bin: executables_bin
                .ok_or_else(|| "--executables-bin is required".to_string())?,
            supervisor_port: supervisor_port
                .ok_or_else(|| "--supervisor-port is required".to_string())?,
        })
    }

    pub(crate) fn supervisor(&self) -> &std::path::Path {
        &self.supervisor
    }

    pub(crate) fn executables_bin(&self) -> &std::path::Path {
        &self.executables_bin
    }

    pub(crate) fn supervisor_port(&self) -> u16 {
        self.supervisor_port
    }
}

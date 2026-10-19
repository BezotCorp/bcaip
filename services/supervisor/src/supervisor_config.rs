use executables_manager::Executable;
use executables_manager::{ExecutableId, ExecutablesManager};
use launch_ecosystem::ExecutionEnvironment;
use std::env;
use std::error::Error;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

const DEFAULT_BIND_ADDRESS: &str = "127.0.0.1:3010";
const DEFAULT_CANDIDATE_BIND_ADDRESS: &str = "127.0.0.1:3011";

#[derive(Clone, Debug)]
pub(crate) struct SupervisorConfig {
    data_directory: PathBuf,
    executable_root: PathBuf,
    supervisor_id: ExecutableId,
    supervisor_name: String,
    bind_address_override: Option<SocketAddr>,
    candidate_bind_address: SocketAddr,
    candidate: bool,
    promoted_address: Option<SocketAddr>,
}

impl SupervisorConfig {
    pub(crate) fn from_env() -> Result<Self, Box<dyn Error>> {
        let data_directory = Self::load_data_directory()?;
        let executable_root = Self::load_executable_root()?;

        let supervisor_id = ExecutableId::parse(
            &env::var("BCAIP_EXECUTABLE_ID")
                .map_err(|_| "BCAIP_EXECUTABLE_ID must identify this supervisor")?,
        )?;
        let supervisor = ExecutablesManager::new(
            &data_directory,
            ExecutionEnvironment::detect(&data_directory)?,
        )
        .resolve_by_id(supervisor_id)?;

        Ok(Self {
            data_directory,
            executable_root,
            supervisor_id,
            supervisor_name: supervisor.name().as_str().to_string(),
            bind_address_override: Self::bind_address_override()?,
            candidate_bind_address: Self::load_candidate_bind_address()?,
            candidate: env::var_os("BCAIP_SUPERVISOR_CANDIDATE").is_some(),
            promoted_address: Self::load_promoted_address()?,
        })
    }

    pub(crate) fn data_directory(&self) -> &PathBuf {
        &self.data_directory
    }

    pub(crate) fn executable_root(&self) -> &Path {
        &self.executable_root
    }

    pub(crate) fn supervisor_name(&self) -> &str {
        &self.supervisor_name
    }

    pub(crate) fn is_candidate(&self) -> bool {
        self.candidate
    }

    /// Address a candidate takes over once promoted: the one the previous
    /// authoritative supervisor served.
    pub(crate) fn promoted_address(&self) -> Option<SocketAddr> {
        self.promoted_address
    }

    pub(crate) fn is_current_supervisor(&self, executable: &Executable) -> bool {
        executable.id() == self.supervisor_id
    }

    pub(crate) fn candidate_bind_address(&self) -> SocketAddr {
        self.candidate_bind_address
    }

    pub(crate) fn bind_address(
        &self,
        self_definition: Option<&Executable>,
    ) -> Result<SocketAddr, Box<dyn Error>> {
        if let Some(address) = self.bind_address_override {
            return Ok(address);
        }

        if let Some(executable) = self_definition {
            return Ok(executable.default_ws_address());
        }

        Ok(DEFAULT_BIND_ADDRESS.parse()?)
    }

    fn bind_address_override() -> Result<Option<SocketAddr>, Box<dyn Error>> {
        if let Some(value) = env::var_os("BCAIP_SUPERVISOR_BIND_ADDRESS") {
            return Ok(Some(value.to_string_lossy().parse()?));
        }

        if let Some(value) = env::var_os("BCAIP_WS_ADDRESS") {
            return Ok(Some(value.to_string_lossy().parse()?));
        }

        Ok(None)
    }

    fn load_promoted_address() -> Result<Option<SocketAddr>, Box<dyn Error>> {
        if env::var_os("BCAIP_SUPERVISOR_CANDIDATE").is_none() {
            return Ok(None);
        }

        match env::var_os("BCAIP_SUPERVISOR_ADDRESS") {
            Some(value) => Ok(Some(value.to_string_lossy().parse()?)),
            None => Err("a supervisor candidate requires BCAIP_SUPERVISOR_ADDRESS".into()),
        }
    }

    fn load_candidate_bind_address() -> Result<SocketAddr, Box<dyn Error>> {
        if let Some(value) = env::var_os("BCAIP_SUPERVISOR_CANDIDATE_BIND_ADDRESS") {
            return Ok(value.to_string_lossy().parse()?);
        }

        Ok(DEFAULT_CANDIDATE_BIND_ADDRESS.parse()?)
    }

    fn load_executable_root() -> Result<PathBuf, Box<dyn Error>> {
        let value =
            env::var_os("BCAIP_EXECUTABLE_ROOT").ok_or("BCAIP_EXECUTABLE_ROOT is required")?;

        let path = PathBuf::from(value);

        if !path.is_absolute() {
            return Err(
                format!("BCAIP_EXECUTABLE_ROOT must be absolute: {}", path.display()).into(),
            );
        }

        Ok(path)
    }

    fn load_data_directory() -> Result<PathBuf, Box<dyn Error>> {
        if let Some(directory) = env::var_os("BCAIP_DATA_DIR") {
            return Ok(PathBuf::from(directory));
        }

        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        let root = manifest
            .parent()
            .and_then(|services| services.parent())
            .ok_or("supervisor manifest has no workspace root")?;

        Ok(root.join("data"))
    }
}

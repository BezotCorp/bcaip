use launch_ecosystem::ExecutionEnvironment;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use files_manager::{AtomicFile, BinaryFile, RonFile};

use crate::{
    executable::Executable, executable_id::ExecutableId,
    executable_index_by_name::ExecutableIndexByName,
    executable_index_by_uuid::ExecutableIndexByUuid, executable_name::ExecutableName,
    executable_role::ExecutableRole, executable_start_phase::ExecutableStartPhase,
};

const INDEX_BY_NAME: &str = "index_by_name";
const INDEX_BY_UUID: &str = "index_by_uuid";

pub struct ExecutablesManager {
    directory: PathBuf,
    environment: ExecutionEnvironment,
}

impl ExecutablesManager {
    pub fn new(data_directory: impl AsRef<Path>, environment: ExecutionEnvironment) -> Self {
        Self {
            directory: data_directory.as_ref().join("executables"),
            environment,
        }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn materialize(&self) -> Result<(), String> {
        fs::create_dir_all(&self.directory).map_err(|error| {
            format!(
                "failed to create executable directory {}: {error}",
                self.directory.display()
            )
        })?;

        let executables = self.read_sources()?;
        let files = self.expected_files(&executables)?;

        self.apply(&executables, &files)
    }

    /// For the side that owns the RON sources (development, CI): guarantees
    /// that the materialized files are exactly what the current RON sources
    /// produce. Never call it where only the `.bin` files exist (a server):
    /// with no source it would materialize an empty set and remove them.
    /// Readers use `get_by_id`, `get_by_name`, `list` and the helpers below,
    /// which only read the `.bin` files.
    ///
    /// Details: every expected `.bin` and index is compared
    /// byte for byte, and `.bin` files without a source are detected. Nothing
    /// is written when everything already matches.
    pub fn ensure_materialized(&self) -> Result<(), String> {
        if !self.directory.exists() {
            return self.materialize();
        }

        let executables = self.read_sources()?;
        let files = self.expected_files(&executables)?;

        if self.stale_binary_executables(&executables)?.is_empty()
            && files
                .iter()
                .all(|(path, expected)| fs::read(path).is_ok_and(|current| &current == expected))
        {
            return Ok(());
        }

        self.apply(&executables, &files)
    }

    /// Resolves an executable that must exist, e.g. the one a process was told to be.
    pub fn resolve_by_id(&self, id: ExecutableId) -> Result<Executable, String> {
        self.get_by_id(id)?
            .ok_or_else(|| format!("unknown executable: {id}"))
    }

    pub fn list_by_start_phase(
        &self,
        start_phase: ExecutableStartPhase,
    ) -> Result<Vec<Executable>, String> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|executable| executable.start_phase() == start_phase)
            .collect())
    }

    pub fn list_by_role(&self, role: ExecutableRole) -> Result<Vec<Executable>, String> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|executable| executable.has_role(role))
            .collect())
    }

    pub fn resolve_unique_by_role(&self, role: ExecutableRole) -> Result<Executable, String> {
        let mut executables = self.list_by_role(role)?;

        match executables.len() {
            1 => Ok(executables.remove(0)),
            count => Err(format!(
                "exactly one executable with role {role:?} is required, found {count}"
            )),
        }
    }

    /// The `.bin` files of the current materialization (every executable and
    /// both indexes), e.g. to ship them to a machine that only reads them.
    pub fn binary_files(&self) -> Result<Vec<PathBuf>, String> {
        let mut files = vec![self.index_by_name_bin_path(), self.index_by_uuid_bin_path()];

        for id in BinaryFile::read::<ExecutableIndexByUuid>(&self.index_by_uuid_bin_path())?.ids() {
            files.push(self.executable_bin_path(id));
        }

        Ok(files)
    }
    pub fn get_by_name(&self, name: &ExecutableName) -> Result<Option<Executable>, String> {
        let index: ExecutableIndexByName = BinaryFile::read(&self.index_by_name_bin_path())?;

        let Some(id) = index.get(name) else {
            return Ok(None);
        };

        self.get_by_id(id)
    }

    pub fn get_by_id(&self, id: ExecutableId) -> Result<Option<Executable>, String> {
        let index: ExecutableIndexByUuid = BinaryFile::read(&self.index_by_uuid_bin_path())?;

        if index.get(id).is_none() {
            return Ok(None);
        }

        BinaryFile::read(&self.executable_bin_path(id)).map(Some)
    }

    pub fn list(&self) -> Result<Vec<Executable>, String> {
        let index: ExecutableIndexByUuid = BinaryFile::read(&self.index_by_uuid_bin_path())?;

        index
            .ids()
            .map(|id| BinaryFile::read(&self.executable_bin_path(id)))
            .collect()
    }

    pub fn write(&self, executable: &Executable) -> Result<(), String> {
        executable.validate()?;

        fs::create_dir_all(&self.directory).map_err(|error| {
            format!(
                "failed to create executable directory {}: {error}",
                self.directory.display()
            )
        })?;

        for current in self.read_sources()? {
            if current.id() != executable.id() && current.name() == executable.name() {
                return Err(format!(
                    "executable name {} is already used by UUID {}",
                    executable.name(),
                    current.id()
                ));
            }
        }

        RonFile::write(&self.executable_ron_path(executable.id()), executable)?;
        self.materialize()
    }

    pub fn remove(&self, id: ExecutableId) -> Result<bool, String> {
        let ron_path = self.executable_ron_path(id);
        let bin_path = self.executable_bin_path(id);

        let existed = ron_path.exists() || bin_path.exists();

        if ron_path.exists() {
            fs::remove_file(&ron_path)
                .map_err(|error| format!("failed to remove {}: {error}", ron_path.display()))?;
        }

        if bin_path.exists() {
            fs::remove_file(&bin_path)
                .map_err(|error| format!("failed to remove {}: {error}", bin_path.display()))?;
        }

        self.materialize()?;

        Ok(existed)
    }

    fn read_sources(&self) -> Result<Vec<Executable>, String> {
        if !self.directory.exists() {
            return Ok(Vec::new());
        }

        let mut executables = Vec::new();
        let mut ids = HashSet::new();

        let entries = fs::read_dir(&self.directory).map_err(|error| {
            format!(
                "failed to read executable directory {}: {error}",
                self.directory.display()
            )
        })?;

        for entry in entries {
            let entry = entry.map_err(|error| {
                format!(
                    "failed to read entry in {}: {error}",
                    self.directory.display()
                )
            })?;

            let path = entry.path();

            if path.extension().and_then(|value| value.to_str()) != Some("ron") {
                continue;
            }

            let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                return Err(format!("invalid executable filename: {}", path.display()));
            };

            let id_text = match self.environment {
                ExecutionEnvironment::Development => {
                    let Some(stem) = file_name.strip_suffix(".local.ron") else {
                        continue;
                    };

                    if stem == INDEX_BY_NAME || stem == INDEX_BY_UUID {
                        continue;
                    }

                    stem
                }

                ExecutionEnvironment::Production => {
                    if file_name.ends_with(".local.ron") {
                        continue;
                    }

                    let Some(stem) = file_name.strip_suffix(".ron") else {
                        continue;
                    };

                    if stem == INDEX_BY_NAME || stem == INDEX_BY_UUID {
                        continue;
                    }

                    stem
                }
            };

            let id = ExecutableId::parse(id_text)?;

            let executable: Executable = RonFile::read(&path)?;

            executable.validate()?;

            if executable.id() != id {
                return Err(format!(
                    "{} contains UUID {} but its filename is {}",
                    path.display(),
                    executable.id(),
                    id
                ));
            }

            if !ids.insert(id) {
                return Err(format!("duplicate executable UUID: {id}"));
            }

            executables.push(executable);
        }

        executables.sort_by_key(Executable::id);

        Ok(executables)
    }

    fn apply(
        &self,
        executables: &[Executable],
        files: &[(PathBuf, Vec<u8>)],
    ) -> Result<(), String> {
        for (path, bytes) in files {
            AtomicFile::write(path, bytes)?;
        }

        for path in self.stale_binary_executables(executables)? {
            fs::remove_file(&path)
                .map_err(|error| format!("failed to remove {}: {error}", path.display()))?;
        }

        Ok(())
    }

    /// Every file the sources must be materialized into, with its exact content.
    fn expected_files(
        &self,
        executables: &[Executable],
    ) -> Result<Vec<(PathBuf, Vec<u8>)>, String> {
        let mut by_name = ExecutableIndexByName::default();
        let mut by_uuid = ExecutableIndexByUuid::default();
        let mut files = Vec::new();

        for executable in executables {
            by_name.insert(executable.name().clone(), executable.id())?;
            by_uuid.insert(executable.id(), executable.name().clone())?;

            files.push((
                self.executable_bin_path(executable.id()),
                BinaryFile::encode(&self.executable_bin_path(executable.id()), executable)?,
            ));
        }

        files.push((
            self.index_by_name_ron_path(),
            RonFile::encode(&self.index_by_name_ron_path(), &by_name)?,
        ));
        files.push((
            self.index_by_name_bin_path(),
            BinaryFile::encode(&self.index_by_name_bin_path(), &by_name)?,
        ));
        files.push((
            self.index_by_uuid_ron_path(),
            RonFile::encode(&self.index_by_uuid_ron_path(), &by_uuid)?,
        ));
        files.push((
            self.index_by_uuid_bin_path(),
            BinaryFile::encode(&self.index_by_uuid_bin_path(), &by_uuid)?,
        ));

        Ok(files)
    }

    fn stale_binary_executables(&self, executables: &[Executable]) -> Result<Vec<PathBuf>, String> {
        let valid: HashSet<String> = executables
            .iter()
            .map(|executable| executable.id().to_string())
            .collect();

        let mut stale = Vec::new();

        let entries = fs::read_dir(&self.directory).map_err(|error| {
            format!(
                "failed to read executable directory {}: {error}",
                self.directory.display()
            )
        })?;

        for entry in entries {
            let entry = entry.map_err(|error| {
                format!(
                    "failed to read entry in {}: {error}",
                    self.directory.display()
                )
            })?;

            let path = entry.path();

            if path.extension().and_then(|value| value.to_str()) != Some("bin") {
                continue;
            }

            let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };

            let id_text = match self.environment {
                ExecutionEnvironment::Development => {
                    let Some(stem) = file_name.strip_suffix(".local.bin") else {
                        continue;
                    };

                    if stem == INDEX_BY_NAME || stem == INDEX_BY_UUID {
                        continue;
                    }

                    stem
                }

                ExecutionEnvironment::Production => {
                    if file_name.ends_with(".local.bin") {
                        continue;
                    }

                    let Some(stem) = file_name.strip_suffix(".bin") else {
                        continue;
                    };

                    if stem == INDEX_BY_NAME || stem == INDEX_BY_UUID {
                        continue;
                    }

                    stem
                }
            };

            if valid.contains(id_text) {
                continue;
            }

            if ExecutableId::parse(id_text).is_ok() {
                stale.push(path);
            }
        }

        Ok(stale)
    }

    fn executable_ron_path(&self, id: ExecutableId) -> PathBuf {
        self.environment
            .resolve(&self.directory.join(format!("{id}.ron")))
    }

    fn executable_bin_path(&self, id: ExecutableId) -> PathBuf {
        self.environment
            .resolve(&self.directory.join(format!("{id}.bin")))
    }

    fn index_by_name_ron_path(&self) -> PathBuf {
        self.environment
            .resolve(&self.directory.join(format!("{INDEX_BY_NAME}.ron")))
    }

    fn index_by_name_bin_path(&self) -> PathBuf {
        self.environment
            .resolve(&self.directory.join(format!("{INDEX_BY_NAME}.bin")))
    }

    fn index_by_uuid_ron_path(&self) -> PathBuf {
        self.environment
            .resolve(&self.directory.join(format!("{INDEX_BY_UUID}.ron")))
    }

    fn index_by_uuid_bin_path(&self) -> PathBuf {
        self.environment
            .resolve(&self.directory.join(format!("{INDEX_BY_UUID}.bin")))
    }
}

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use bincode_next::{Decode, Encode, decode_from_slice, encode_to_vec};

use crate::{
    atomic_file::AtomicFile, executable::Executable, executable_id::ExecutableId,
    executable_index_by_name::ExecutableIndexByName,
    executable_index_by_uuid::ExecutableIndexByUuid, executable_name::ExecutableName,
};

const INDEX_BY_NAME: &str = "index_by_name";
const INDEX_BY_UUID: &str = "index_by_uuid";

pub struct ExecutablesManager {
    directory: PathBuf,
}

impl ExecutablesManager {
    pub fn new(data_directory: impl AsRef<Path>) -> Self {
        Self {
            directory: data_directory.as_ref().join("executables"),
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

        let mut by_name = ExecutableIndexByName::default();
        let mut by_uuid = ExecutableIndexByUuid::default();

        for executable in &executables {
            by_name.insert(executable.name().clone(), executable.id())?;
            by_uuid.insert(executable.id(), executable.name().clone())?;

            self.write_binary_executable(executable)?;
        }

        self.remove_stale_binary_executables(&executables)?;

        self.write_ron(self.index_by_name_ron_path(), &by_name)?;
        self.write_binary(self.index_by_name_bin_path(), &by_name)?;

        self.write_ron(self.index_by_uuid_ron_path(), &by_uuid)?;
        self.write_binary(self.index_by_uuid_bin_path(), &by_uuid)
    }

    pub fn get_by_name(&self, name: &ExecutableName) -> Result<Option<Executable>, String> {
        let index: ExecutableIndexByName = self.read_binary(self.index_by_name_bin_path())?;

        let Some(id) = index.get(name) else {
            return Ok(None);
        };

        self.get_by_id(id)
    }

    pub fn get_by_id(&self, id: ExecutableId) -> Result<Option<Executable>, String> {
        let index: ExecutableIndexByUuid = self.read_binary(self.index_by_uuid_bin_path())?;

        if index.get(id).is_none() {
            return Ok(None);
        }

        self.read_binary(self.executable_bin_path(id)).map(Some)
    }

    pub fn list(&self) -> Result<Vec<Executable>, String> {
        let index: ExecutableIndexByUuid = self.read_binary(self.index_by_uuid_bin_path())?;

        index
            .ids()
            .map(|id| self.read_binary(self.executable_bin_path(id)))
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

        self.write_ron(self.executable_ron_path(executable.id()), executable)?;
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

            let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
                return Err(format!("invalid executable filename: {}", path.display()));
            };

            if stem == INDEX_BY_NAME || stem == INDEX_BY_UUID {
                continue;
            }

            let id = ExecutableId::parse(stem)?;

            let source = fs::read_to_string(&path)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

            let executable: Executable = ron::from_str(&source)
                .map_err(|error| format!("failed to decode {}: {error}", path.display()))?;

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

    fn remove_stale_binary_executables(&self, executables: &[Executable]) -> Result<(), String> {
        let valid: HashSet<String> = executables
            .iter()
            .map(|executable| executable.id().to_string())
            .collect();

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

            let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };

            if stem == INDEX_BY_NAME || stem == INDEX_BY_UUID || valid.contains(stem) {
                continue;
            }

            if ExecutableId::parse(stem).is_ok() {
                fs::remove_file(&path)
                    .map_err(|error| format!("failed to remove {}: {error}", path.display()))?;
            }
        }

        Ok(())
    }

    fn executable_ron_path(&self, id: ExecutableId) -> PathBuf {
        self.directory.join(format!("{id}.ron"))
    }

    fn executable_bin_path(&self, id: ExecutableId) -> PathBuf {
        self.directory.join(format!("{id}.bin"))
    }

    fn index_by_name_ron_path(&self) -> PathBuf {
        self.directory.join(format!("{INDEX_BY_NAME}.ron"))
    }

    fn index_by_name_bin_path(&self) -> PathBuf {
        self.directory.join(format!("{INDEX_BY_NAME}.bin"))
    }

    fn index_by_uuid_ron_path(&self) -> PathBuf {
        self.directory.join(format!("{INDEX_BY_UUID}.ron"))
    }

    fn index_by_uuid_bin_path(&self) -> PathBuf {
        self.directory.join(format!("{INDEX_BY_UUID}.bin"))
    }

    fn write_ron<T>(&self, path: PathBuf, value: &T) -> Result<(), String>
    where
        T: serde::Serialize,
    {
        let serialized = ron::ser::to_string_pretty(value, ron::ser::PrettyConfig::default())
            .map_err(|error| format!("failed to encode {}: {error}", path.display()))?;

        AtomicFile::write(&path, serialized.as_bytes())
    }

    fn write_binary<T>(&self, path: PathBuf, value: &T) -> Result<(), String>
    where
        T: Encode,
    {
        let encoded = encode_to_vec(value, bincode_next::config::standard())
            .map_err(|error| format!("failed to encode {}: {error}", path.display()))?;

        AtomicFile::write(&path, &encoded)
    }

    fn write_binary_executable(&self, executable: &Executable) -> Result<(), String> {
        self.write_binary(self.executable_bin_path(executable.id()), executable)
    }

    fn read_binary<T>(&self, path: PathBuf) -> Result<T, String>
    where
        T: Decode<()>,
    {
        let bytes = fs::read(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

        let (value, consumed) = decode_from_slice::<T, _>(&bytes, bincode_next::config::standard())
            .map_err(|error| format!("failed to decode {}: {error}", path.display()))?;

        if consumed != bytes.len() {
            return Err(format!("{} contains trailing bytes", path.display()));
        }

        Ok(value)
    }
}

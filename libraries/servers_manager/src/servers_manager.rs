use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use bincode_next::{Decode, Encode, decode_from_slice, encode_to_vec};
use launch_ecosystem::ExecutionEnvironment;

use crate::{
    AtomicFile, Server, ServerDeploymentStatus, ServerId, ServerIndexByName, ServerIndexByUuid,
    ServerIndexEntry, ServerName,
};

const INDEX_BY_NAME: &str = "index_by_name";
const INDEX_BY_UUID: &str = "index_by_uuid";

pub struct ServersManager {
    directory: PathBuf,
    environment: ExecutionEnvironment,
}

impl ServersManager {
    pub fn new(data_directory: impl AsRef<Path>, environment: ExecutionEnvironment) -> Self {
        Self {
            directory: data_directory.as_ref().join("servers"),
            environment,
        }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn materialize(&self) -> Result<(), String> {
        fs::create_dir_all(&self.directory).map_err(|error| {
            format!(
                "failed to create server directory {}: {error}",
                self.directory.display()
            )
        })?;

        let servers = self.read_sources()?;
        let files = self.expected_files(&servers)?;

        self.apply(&servers, &files)
    }

    pub fn ensure_materialized(&self) -> Result<(), String> {
        if !self.directory.exists() {
            return self.materialize();
        }

        let servers = self.read_sources()?;
        let files = self.expected_files(&servers)?;

        if self.stale_binary_servers(&servers)?.is_empty()
            && files
                .iter()
                .all(|(path, expected)| fs::read(path).is_ok_and(|current| &current == expected))
        {
            return Ok(());
        }

        self.apply(&servers, &files)
    }

    pub fn resolve_by_id(&self, id: ServerId) -> Result<Server, String> {
        self.get_by_id(id)?
            .ok_or_else(|| format!("unknown server: {id}"))
    }

    pub fn resolve_by_name(&self, name: &ServerName) -> Result<Server, String> {
        self.get_by_name(name)?
            .ok_or_else(|| format!("unknown server: {name}"))
    }

    pub fn index_by_name(&self) -> Result<ServerIndexByName, String> {
        self.read_binary(self.index_by_name_bin_path())
    }

    pub fn index_by_uuid(&self) -> Result<ServerIndexByUuid, String> {
        self.read_binary(self.index_by_uuid_bin_path())
    }

    pub fn binary_files(&self) -> Result<Vec<PathBuf>, String> {
        let mut files = vec![self.index_by_name_bin_path(), self.index_by_uuid_bin_path()];

        for id in self.index_by_uuid()?.ids() {
            files.push(self.server_bin_path(id));
        }

        Ok(files)
    }

    pub fn get_by_name(&self, name: &ServerName) -> Result<Option<Server>, String> {
        let index = self.index_by_name()?;

        let Some(entry) = index.get(name) else {
            return Ok(None);
        };

        self.get_by_id(entry.id())
    }

    pub fn get_by_id(&self, id: ServerId) -> Result<Option<Server>, String> {
        let index = self.index_by_uuid()?;

        if index.get(id).is_none() {
            return Ok(None);
        }

        self.read_binary(self.server_bin_path(id)).map(Some)
    }

    pub fn list(&self) -> Result<Vec<Server>, String> {
        let index = self.index_by_uuid()?;
        let mut servers = Vec::new();

        for id in index.ids() {
            servers.push(self.read_binary(self.server_bin_path(id))?);
        }

        Ok(servers)
    }

    pub fn set_deployment_status(
        &self,
        id: ServerId,
        deployment_status: ServerDeploymentStatus,
    ) -> Result<(), String> {
        let mut by_name = self.index_by_name()?;
        let mut by_uuid = self.index_by_uuid()?;

        let name = by_uuid
            .get(id)
            .map(|entry| entry.name().clone())
            .ok_or_else(|| format!("unknown server: {id}"))?;

        let by_uuid_entry = by_uuid
            .get_mut(id)
            .ok_or_else(|| format!("server UUID index entry disappeared: {id}"))?;

        by_uuid_entry.set_deployment_status(deployment_status);

        let by_name_entry = by_name
            .get_mut(&name)
            .ok_or_else(|| format!("server name index entry missing: {name}"))?;

        if by_name_entry.id() != id {
            return Err(format!(
                "server indexes disagree for {}: name index has {}, UUID index has {}",
                name,
                by_name_entry.id(),
                id
            ));
        }

        by_name_entry.set_deployment_status(deployment_status);

        self.write_binary(self.index_by_name_bin_path(), &by_name)?;
        self.write_binary(self.index_by_uuid_bin_path(), &by_uuid)
    }

    pub fn set_deployment_status_by_name(
        &self,
        name: &ServerName,
        deployment_status: ServerDeploymentStatus,
    ) -> Result<(), String> {
        let index = self.index_by_name()?;

        let id = index
            .get(name)
            .map(ServerIndexEntry::id)
            .ok_or_else(|| format!("unknown server: {name}"))?;

        self.set_deployment_status(id, deployment_status)
    }

    fn read_sources(&self) -> Result<Vec<Server>, String> {
        if !self.directory.exists() {
            return Ok(Vec::new());
        }

        let mut servers = Vec::new();

        for entry in fs::read_dir(&self.directory).map_err(|error| {
            format!(
                "failed to read server directory {}: {error}",
                self.directory.display()
            )
        })? {
            let entry = entry.map_err(|error| {
                format!(
                    "failed to read entry in {}: {error}",
                    self.directory.display()
                )
            })?;

            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                return Err(format!("invalid server filename: {}", path.display()));
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

            let id = ServerId::parse(id_text)?;

            let content = fs::read_to_string(&path)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

            let server: Server = ron::from_str(&content)
                .map_err(|error| format!("failed to decode {}: {error}", path.display()))?;

            server.validate()?;

            if server.id() != id {
                return Err(format!(
                    "server UUID {} does not match filename UUID {} in {}",
                    server.id(),
                    id,
                    path.display()
                ));
            }

            servers.push(server);
        }

        Ok(servers)
    }

    fn apply(&self, servers: &[Server], files: &[(PathBuf, Vec<u8>)]) -> Result<(), String> {
        for (path, bytes) in files {
            AtomicFile::write(path, bytes)?;
        }

        for path in self.stale_binary_servers(servers)? {
            fs::remove_file(&path)
                .map_err(|error| format!("failed to remove {}: {error}", path.display()))?;
        }

        Ok(())
    }

    fn expected_files(&self, servers: &[Server]) -> Result<Vec<(PathBuf, Vec<u8>)>, String> {
        let previous_statuses = self.previous_deployment_statuses()?;

        let mut by_name = ServerIndexByName::default();
        let mut by_uuid = ServerIndexByUuid::default();
        let mut files = Vec::new();

        for server in servers {
            let deployment_status = previous_statuses
                .get(&server.id())
                .copied()
                .unwrap_or_default();

            let entry = ServerIndexEntry::new(
                server.id(),
                server.name().clone(),
                server.is_deployment_target(),
                deployment_status,
            );

            by_name.insert(entry.clone())?;
            by_uuid.insert(entry)?;

            let path = self.server_bin_path(server.id());

            files.push((path.clone(), Self::encode_binary(&path, server)?));
        }

        let index_by_name_ron = self.index_by_name_ron_path();
        files.push((
            index_by_name_ron.clone(),
            Self::encode_ron(&index_by_name_ron, &by_name)?,
        ));

        let index_by_name_bin = self.index_by_name_bin_path();
        files.push((
            index_by_name_bin.clone(),
            Self::encode_binary(&index_by_name_bin, &by_name)?,
        ));

        let index_by_uuid_ron = self.index_by_uuid_ron_path();
        files.push((
            index_by_uuid_ron.clone(),
            Self::encode_ron(&index_by_uuid_ron, &by_uuid)?,
        ));

        let index_by_uuid_bin = self.index_by_uuid_bin_path();
        files.push((
            index_by_uuid_bin.clone(),
            Self::encode_binary(&index_by_uuid_bin, &by_uuid)?,
        ));

        Ok(files)
    }

    fn previous_deployment_statuses(
        &self,
    ) -> Result<HashMap<ServerId, ServerDeploymentStatus>, String> {
        let path = self.index_by_uuid_bin_path();

        if !path.exists() {
            return Ok(HashMap::new());
        }

        let index: ServerIndexByUuid = self.read_binary(path)?;

        Ok(index
            .entries()
            .map(|(id, entry)| (id, entry.deployment_status()))
            .collect())
    }

    fn stale_binary_servers(&self, servers: &[Server]) -> Result<Vec<PathBuf>, String> {
        if !self.directory.exists() {
            return Ok(Vec::new());
        }

        let valid: HashSet<String> = servers
            .iter()
            .map(|server| server.id().to_string())
            .collect();

        let mut stale = Vec::new();

        for entry in fs::read_dir(&self.directory).map_err(|error| {
            format!(
                "failed to read server directory {}: {error}",
                self.directory.display()
            )
        })? {
            let entry = entry.map_err(|error| {
                format!(
                    "failed to read entry in {}: {error}",
                    self.directory.display()
                )
            })?;

            let path = entry.path();

            if !path.is_file() {
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

            if ServerId::parse(id_text).is_err() {
                continue;
            }

            if !valid.contains(id_text) {
                stale.push(path);
            }
        }

        Ok(stale)
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
            return Err(format!("{} contains trailing binary data", path.display()));
        }

        Ok(value)
    }

    fn write_binary<T>(&self, path: PathBuf, value: &T) -> Result<(), String>
    where
        T: Encode,
    {
        let bytes = Self::encode_binary(&path, value)?;

        AtomicFile::write(&path, &bytes)
    }

    fn encode_binary<T>(path: &Path, value: &T) -> Result<Vec<u8>, String>
    where
        T: Encode,
    {
        encode_to_vec(value, bincode_next::config::standard())
            .map_err(|error| format!("failed to encode {}: {error}", path.display()))
    }

    fn encode_ron<T>(path: &Path, value: &T) -> Result<Vec<u8>, String>
    where
        T: serde::Serialize,
    {
        let content = ron::ser::to_string_pretty(value, ron::ser::PrettyConfig::default())
            .map_err(|error| format!("failed to encode {}: {error}", path.display()))?;

        Ok(content.into_bytes())
    }

    fn server_bin_path(&self, id: ServerId) -> PathBuf {
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

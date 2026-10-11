use crate::begin_update_result::BeginUpdateResult;
use crate::executable::Executable;
use crate::process::Process;
use crate::update::Update;
use crate::update_phase::UpdatePhase;
use bincode_next::{decode_from_slice, encode_to_vec};
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) struct ExecutableStore {
    source_path: PathBuf,
    runtime_path: PathBuf,
    executables: Mutex<Vec<Executable>>,
    authoritative_instances: Mutex<HashMap<String, String>>,
}

impl ExecutableStore {
    pub(crate) fn load(data_directory: &Path) -> Result<Self, String> {
        fs::create_dir_all(data_directory).map_err(|error| {
            format!(
                "failed to create data directory {}: {error}",
                data_directory.display()
            )
        })?;

        let local_source = data_directory.join("executables.local.ron");
        let production_source = data_directory.join("executables.ron");

        let source_path = if local_source.exists() {
            local_source
        } else {
            production_source
        };

        let runtime_path = data_directory.join("executables.bin");

        let source = fs::read_to_string(&source_path)
            .map_err(|error| format!("failed to read {}: {error}", source_path.display()))?;

        let mut executables: Vec<Executable> = ron::from_str(&source)
            .map_err(|error| format!("failed to decode {}: {error}", source_path.display()))?;

        validate_definitions(&executables)?;

        for executable in &executables {
            if executable.process.is_some() || executable.update.is_some() {
                return Err(format!(
                    "{} source definition must have process = None and update = None",
                    executable.name
                ));
            }
        }

        if let Ok(bytes) = fs::read(&runtime_path) {
            let config = bincode_next::config::standard();

            if let Ok((previous, consumed)) =
                decode_from_slice::<Vec<Executable>, _>(&bytes, config)
            {
                if consumed == bytes.len() {
                    merge_runtime_state(&mut executables, previous);
                }
            }
        }

        let authoritative_instances = executables
            .iter()
            .filter_map(|executable| {
                executable
                    .process
                    .as_ref()
                    .map(|process| (executable.name.clone(), process.instance_id.clone()))
            })
            .collect();

        let store = Self {
            source_path,
            runtime_path,
            executables: Mutex::new(executables),
            authoritative_instances: Mutex::new(authoritative_instances),
        };

        store.persist()?;

        Ok(store)
    }

    pub(crate) fn source_path(&self) -> &Path {
        &self.source_path
    }

    pub(crate) fn snapshot(&self) -> Result<Vec<Executable>, String> {
        self.executables
            .lock()
            .map(|executables| executables.clone())
            .map_err(|_| "executables store lock poisoned".to_string())
    }

    pub(crate) fn get(&self, name: &str) -> Result<Option<Executable>, String> {
        let executables = self
            .executables
            .lock()
            .map_err(|_| "executables store lock poisoned".to_string())?;

        Ok(executables
            .iter()
            .find(|executable| executable.name == name)
            .cloned())
    }

    pub(crate) fn expect_process(&self, name: &str, instance_id: &str) -> Result<(), String> {
        {
            let executables = self
                .executables
                .lock()
                .map_err(|_| "executables store lock poisoned".to_string())?;

            executable_index(&executables, name)?;
        }

        self.authoritative_instances
            .lock()
            .map_err(|_| "authoritative instances lock poisoned".to_string())?
            .insert(name.to_string(), instance_id.to_string());

        Ok(())
    }

    pub(crate) fn set_process(&self, name: &str, process: Process) -> Result<(), String> {
        let mut executables = self
            .executables
            .lock()
            .map_err(|_| "executables store lock poisoned".to_string())?;

        let index = executable_index(&executables, name)?;

        let mut authoritative_instances = self
            .authoritative_instances
            .lock()
            .map_err(|_| "authoritative instances lock poisoned".to_string())?;

        if let Some(expected) = authoritative_instances.get(name) {
            if expected != &process.instance_id {
                return Err(format!(
                    "{} rejected process instance {}; authoritative instance is {}",
                    name, process.instance_id, expected
                ));
            }
        }

        let previous_process = executables[index].process.clone();

        let previous_authoritative =
            authoritative_instances.insert(name.to_string(), process.instance_id.clone());

        executables[index].process = Some(process);

        if let Err(error) = persist_snapshot(&self.runtime_path, &executables) {
            executables[index].process = previous_process;

            match previous_authoritative {
                Some(instance_id) => {
                    authoritative_instances.insert(name.to_string(), instance_id);
                }

                None => {
                    authoritative_instances.remove(name);
                }
            }

            return Err(error);
        }

        Ok(())
    }

    pub(crate) fn clear_process(&self, name: &str, instance_id: &str) -> Result<(), String> {
        let mut executables = self
            .executables
            .lock()
            .map_err(|_| "executables store lock poisoned".to_string())?;

        let index = executable_index(&executables, name)?;

        let matches = executables[index]
            .process
            .as_ref()
            .is_some_and(|process| process.instance_id == instance_id);

        if !matches {
            return Ok(());
        }

        let mut authoritative_instances = self
            .authoritative_instances
            .lock()
            .map_err(|_| "authoritative instances lock poisoned".to_string())?;

        let previous_process = executables[index].process.take();

        let previous_authoritative = if authoritative_instances
            .get(name)
            .is_some_and(|expected| expected == instance_id)
        {
            authoritative_instances.remove(name)
        } else {
            None
        };

        if let Err(error) = persist_snapshot(&self.runtime_path, &executables) {
            executables[index].process = previous_process;

            if let Some(instance_id) = previous_authoritative {
                authoritative_instances.insert(name.to_string(), instance_id);
            }

            return Err(error);
        }

        Ok(())
    }

    pub(crate) fn begin_update(
        &self,
        name: &str,
        update_id: String,
        expected_size: u64,
        sha256: String,
    ) -> Result<BeginUpdateResult, String> {
        let mut executables = self
            .executables
            .lock()
            .map_err(|_| "executables store lock poisoned".to_string())?;

        let index = executables
            .iter()
            .position(|executable| executable.name == name)
            .ok_or_else(|| format!("unknown executable: {name}"))?;

        if let Some(existing) = executables[index].update.as_ref() {
            if existing.phase == UpdatePhase::Pending {
                if existing.update_id == update_id {
                    if existing.expected_size != expected_size
                        || !existing.sha256.eq_ignore_ascii_case(&sha256)
                    {
                        return Err("update_id already exists with different metadata".to_string());
                    }

                    return Ok(BeginUpdateResult::AlreadyPending);
                }

                return Err(format!(
                    "{} already has pending update {}",
                    name, existing.update_id
                ));
            }

            if existing.update_id == update_id {
                if existing.expected_size != expected_size
                    || !existing.sha256.eq_ignore_ascii_case(&sha256)
                {
                    return Err("update_id already exists with different metadata".to_string());
                }

                if existing.phase == UpdatePhase::Committed {
                    return Ok(BeginUpdateResult::AlreadyCommitted);
                }
            }
        }

        let previous = executables[index].update.clone();

        executables[index].update = Some(Update {
            update_id,
            phase: UpdatePhase::Pending,
            expected_size,
            received_size: 0,
            sha256,
            error: None,
        });

        if let Err(error) = persist_snapshot(&self.runtime_path, &executables) {
            executables[index].update = previous;
            return Err(error);
        }

        Ok(BeginUpdateResult::Started)
    }

    pub(crate) fn set_update(&self, name: &str, update: Update) -> Result<(), String> {
        let mut executables = self
            .executables
            .lock()
            .map_err(|_| "executables store lock poisoned".to_string())?;

        let index = executable_index(&executables, name)?;

        let previous = executables[index].update.clone();

        executables[index].update = Some(update);

        if let Err(error) = persist_snapshot(&self.runtime_path, &executables) {
            executables[index].update = previous;

            return Err(error);
        }

        Ok(())
    }

    fn persist(&self) -> Result<(), String> {
        let executables = self
            .executables
            .lock()
            .map_err(|_| "executables store lock poisoned".to_string())?
            .clone();

        let config = bincode_next::config::standard();

        let encoded = encode_to_vec(&executables, config)
            .map_err(|error| format!("failed to encode executables runtime: {error}"))?;

        atomic_write(&self.runtime_path, &encoded)
    }
}

fn executable_index(executables: &[Executable], name: &str) -> Result<usize, String> {
    executables
        .iter()
        .position(|executable| executable.name == name)
        .ok_or_else(|| format!("unknown executable: {name}"))
}

fn persist_snapshot(runtime_path: &Path, executables: &[Executable]) -> Result<(), String> {
    let config = bincode_next::config::standard();

    let encoded = encode_to_vec(executables, config)
        .map_err(|error| format!("failed to encode executables runtime: {error}"))?;

    atomic_write(runtime_path, &encoded)
}

fn validate_definitions(executables: &[Executable]) -> Result<(), String> {
    let mut names = HashSet::new();
    let mut default_addresses = HashSet::new();

    for executable in executables {
        executable.validate_definition()?;

        if !names.insert(executable.name.as_str()) {
            return Err(format!("duplicate executable: {}", executable.name));
        }

        if !default_addresses.insert(executable.default_ws_address) {
            return Err(format!(
                "duplicate default WebSocket address: {}",
                executable.default_ws_address
            ));
        }
    }

    Ok(())
}

fn merge_runtime_state(definitions: &mut [Executable], previous: Vec<Executable>) {
    let previous: HashMap<String, Executable> = previous
        .into_iter()
        .map(|executable| (executable.name.clone(), executable))
        .collect();

    for definition in definitions {
        let Some(old) = previous.get(&definition.name) else {
            continue;
        };

        if old.working_directory != definition.working_directory
            || old.executable_path != definition.executable_path
        {
            continue;
        }

        definition.process = old.process.clone();
        definition.update = old.update.clone();
    }
}

fn atomic_write(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let directory = target
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", target.display()))?;

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    let temp = directory.join(format!(".executables.{}.{}.tmp", std::process::id(), stamp));

    let result = (|| -> Result<(), String> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|error| format!("failed to create {}: {error}", temp.display()))?;

        file.write_all(bytes)
            .map_err(|error| format!("failed to write {}: {error}", temp.display()))?;

        file.sync_all()
            .map_err(|error| format!("failed to sync {}: {error}", temp.display()))?;

        fs::rename(&temp, target)
            .map_err(|error| format!("failed to replace {}: {error}", target.display()))?;

        fs::File::open(directory)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| {
                format!("failed to sync directory {}: {error}", directory.display())
            })?;

        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }

    result
}

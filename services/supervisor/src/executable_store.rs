use crate::handover_journal::HandoverJournal;
use crate::managed_process::ManagedProcess;
use crate::runtime_handover::RuntimeHandover;
use executables_manager::AuthorityState;
use executables_manager::BeginUpdateResult;
use executables_manager::Executable;
use executables_manager::ExecutableRuntime;
use executables_manager::Process;
use executables_manager::RuntimeStateBackup;
use executables_manager::Update;
use executables_manager::{ExecutableId, ExecutablesManager, ExecutionEnvironment};
use files_manager::AtomicFile;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

const PROMOTION_LOCK_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) struct ExecutableStore {
    source_path: PathBuf,
    runtime_path: PathBuf,
    executables: Mutex<Vec<Executable>>,
    authoritative_instances: Mutex<HashMap<ExecutableId, String>>,
    /// Runtime state of executables without a definition whose process may
    /// still be alive. Kept untouched (and persisted) until it is gone.
    orphans: Vec<ExecutableRuntime>,
    authority: AuthorityState,
    write_gate: Mutex<()>,
}

impl ExecutableStore {
    pub(crate) fn load(data_directory: &Path, authority: AuthorityState) -> Result<Self, String> {
        fs::create_dir_all(data_directory).map_err(|error| {
            format!(
                "failed to create data directory {}: {error}",
                data_directory.display()
            )
        })?;

        let definitions = ExecutablesManager::new(
            data_directory,
            ExecutionEnvironment::detect(data_directory)?,
        );

        let definitions_list = definitions.list()?;

        let source_path = definitions.directory().to_path_buf();
        let runtime_path = Self::runtime_path(data_directory);

        let mut runtimes = Self::read_runtime_state(&runtime_path)?;

        let executables: Vec<Executable> = definitions_list
            .into_iter()
            .map(|definition| match runtimes.remove(&definition.id()) {
                Some(runtime) => Executable::with_runtime(definition, runtime),
                None => definition,
            })
            .collect();

        Self::validate_definitions(&executables)?;

        let authoritative_instances = executables
            .iter()
            .filter_map(|executable| {
                executable
                    .process_instance_id_owned()
                    .map(|instance_id| (executable.id(), instance_id))
            })
            .collect();

        let orphans = Self::retain_orphans(runtimes.into_values());

        let store = Self {
            source_path,
            runtime_path,
            executables: Mutex::new(executables),
            authoritative_instances: Mutex::new(authoritative_instances),
            orphans,
            authority,
            write_gate: Mutex::new(()),
        };

        if store.authority.can_write() {
            store.persist()?;
        }

        Ok(store)
    }

    pub(crate) fn runtime_path(data_directory: &Path) -> PathBuf {
        data_directory.join("executables.bin")
    }

    pub(crate) fn source_path(&self) -> &Path {
        &self.source_path
    }

    pub(crate) fn can_write(&self) -> bool {
        self.authority.can_write()
    }

    pub(crate) fn begin_handover(
        &self,
        executable: &str,
        update_id: &str,
    ) -> Result<RuntimeHandover<'_>, String> {
        let gate = self.gate()?;

        self.ensure_writer()?;

        let data_directory = self
            .runtime_path
            .parent()
            .ok_or_else(|| format!("{} has no parent directory", self.runtime_path.display()))?;
        let backup = RuntimeStateBackup::capture(&self.runtime_path)?;
        let journal = HandoverJournal::begin(data_directory, update_id, executable)?;

        Ok(RuntimeHandover::new(self, gate, backup, journal))
    }

    pub(crate) fn release_authority(&self) {
        self.authority.release();
    }

    pub(crate) fn reclaim_authority(&self, timeout: Duration) -> Result<(), String> {
        self.authority.acquire(timeout)
    }

    /// Takes the OS-level write authority, records this process as the
    /// authoritative instance and persists the state.
    pub(crate) fn promote_candidate(&self, name: &str, process: Process) -> Result<(), String> {
        let _gate = self.gate()?;

        if self.authority.can_write() {
            return Err("this process already holds the write authority".to_string());
        }

        {
            let executables = self
                .executables
                .lock()
                .map_err(|_| "executables store lock poisoned".to_string())?;

            Self::executable_index(&executables, name)?;
        }

        self.authority.acquire(PROMOTION_LOCK_TIMEOUT)?;

        let result = self.record_promoted_process(name, process);

        if result.is_err() {
            self.authority.release();
        }

        result
    }

    pub(crate) fn runtime_fingerprint(&self) -> Result<String, String> {
        let executables = self
            .executables
            .lock()
            .map_err(|_| "executables store lock poisoned".to_string())?;
        let mut hasher = Sha256::new();

        for executable in executables.iter() {
            hasher.update(executable.runtime().encode()?);
        }

        Ok(hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }

    pub(crate) fn reject_update(
        &self,
        name: &str,
        update_id: &str,
        error: String,
    ) -> Result<(), String> {
        let _gate = self.gate()?;

        self.reject_update_while_gated(name, update_id, error)
    }

    /// For the holder of the write gate (a handover), which cannot take it again.
    pub(crate) fn reject_update_while_gated(
        &self,
        name: &str,
        update_id: &str,
        error: String,
    ) -> Result<(), String> {
        self.ensure_writer()?;

        let mut executables = self
            .executables
            .lock()
            .map_err(|_| "executables store lock poisoned".to_string())?;
        let index = Self::executable_index(&executables, name)?;

        let Some(mut update) = executables[index].update_owned() else {
            return Ok(());
        };

        if !update.has_id(update_id) {
            return Ok(());
        }

        let previous = Some(update.clone());

        update.fail(error);
        executables[index].replace_update_state(update);

        if let Err(error) = self.persist_snapshot(&executables) {
            executables[index].recover_update_after_persist_failure(previous);

            return Err(error);
        }

        Ok(())
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
            .find(|executable| executable.is_named(name))
            .cloned())
    }

    pub(crate) fn expect_process(&self, name: &str, instance_id: &str) -> Result<(), String> {
        let _gate = self.gate()?;

        self.ensure_writer()?;

        let id = {
            let executables = self
                .executables
                .lock()
                .map_err(|_| "executables store lock poisoned".to_string())?;

            executables[Self::executable_index(&executables, name)?].id()
        };

        self.authoritative_instances
            .lock()
            .map_err(|_| "authoritative instances lock poisoned".to_string())?
            .insert(id, instance_id.to_string());

        Ok(())
    }

    pub(crate) fn set_process(&self, name: &str, process: Process) -> Result<(), String> {
        let _gate = self.gate()?;

        self.ensure_writer()?;

        let mut executables = self
            .executables
            .lock()
            .map_err(|_| "executables store lock poisoned".to_string())?;

        let index = Self::executable_index(&executables, name)?;

        let mut authoritative_instances = self
            .authoritative_instances
            .lock()
            .map_err(|_| "authoritative instances lock poisoned".to_string())?;

        let id = executables[index].id();

        if let Some(expected) = authoritative_instances.get(&id) {
            if !process.has_instance_id(expected) {
                return Err(format!(
                    "{} rejected process instance {}; authoritative instance is {}",
                    name,
                    process.instance_id(),
                    expected
                ));
            }
        }

        let previous_process = executables[index].process().cloned();

        let previous_authoritative =
            authoritative_instances.insert(id, process.instance_id_owned());

        executables[index].record_started_process(process);

        if let Err(error) = self.persist_snapshot(&executables) {
            executables[index].recover_process_after_persist_failure(previous_process);

            match previous_authoritative {
                Some(instance_id) => {
                    authoritative_instances.insert(id, instance_id);
                }

                None => {
                    authoritative_instances.remove(&id);
                }
            }

            return Err(error);
        }

        Ok(())
    }

    pub(crate) fn clear_process(&self, name: &str, instance_id: &str) -> Result<(), String> {
        let _gate = self.gate()?;

        self.ensure_writer()?;

        let mut executables = self
            .executables
            .lock()
            .map_err(|_| "executables store lock poisoned".to_string())?;

        let index = Self::executable_index(&executables, name)?;

        let matches = executables[index].has_process_instance(instance_id);

        if !matches {
            return Ok(());
        }

        let mut authoritative_instances = self
            .authoritative_instances
            .lock()
            .map_err(|_| "authoritative instances lock poisoned".to_string())?;

        let previous_process = executables[index].remove_process();

        let id = executables[index].id();
        let previous_authoritative = if authoritative_instances
            .get(&id)
            .is_some_and(|expected| expected == instance_id)
        {
            authoritative_instances.remove(&id)
        } else {
            None
        };

        if let Err(error) = self.persist_snapshot(&executables) {
            executables[index].recover_process_after_persist_failure(previous_process);

            if let Some(instance_id) = previous_authoritative {
                authoritative_instances.insert(id, instance_id);
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
        let _gate = self.gate()?;

        self.ensure_writer()?;

        let mut executables = self
            .executables
            .lock()
            .map_err(|_| "executables store lock poisoned".to_string())?;

        let index = executables
            .iter()
            .position(|executable| executable.is_named(name))
            .ok_or_else(|| format!("unknown executable: {name}"))?;

        if let Some(existing) = executables[index].update() {
            if existing.is_pending() {
                if existing.has_id(&update_id) {
                    if !existing.matches(&update_id, expected_size, &sha256) {
                        return Err("update_id already exists with different metadata".to_string());
                    }

                    return Ok(BeginUpdateResult::AlreadyPending);
                }

                return Err(format!(
                    "{} already has pending update {}",
                    name,
                    existing.update_id()
                ));
            }

            if existing.has_id(&update_id) {
                if !existing.matches(&update_id, expected_size, &sha256) {
                    return Err("update_id already exists with different metadata".to_string());
                }

                if existing.is_committed() {
                    return Ok(BeginUpdateResult::AlreadyCommitted);
                }
            }
        }

        let previous = executables[index].update_owned();

        executables[index].replace_update_state(Update::new(update_id, expected_size, sha256));

        if let Err(error) = self.persist_snapshot(&executables) {
            executables[index].recover_update_after_persist_failure(previous);
            return Err(error);
        }

        Ok(BeginUpdateResult::Started)
    }

    pub(crate) fn set_update(&self, name: &str, update: Update) -> Result<(), String> {
        let _gate = self.gate()?;

        self.ensure_writer()?;

        let mut executables = self
            .executables
            .lock()
            .map_err(|_| "executables store lock poisoned".to_string())?;

        let index = Self::executable_index(&executables, name)?;

        let previous = executables[index].update_owned();

        executables[index].replace_update_state(update);

        if let Err(error) = self.persist_snapshot(&executables) {
            executables[index].recover_update_after_persist_failure(previous);

            return Err(error);
        }

        Ok(())
    }

    fn persist(&self) -> Result<(), String> {
        self.ensure_writer()?;

        let executables = self
            .executables
            .lock()
            .map_err(|_| "executables store lock poisoned".to_string())?;

        self.persist_snapshot(&executables)
    }

    fn record_promoted_process(&self, name: &str, process: Process) -> Result<(), String> {
        let mut executables = self
            .executables
            .lock()
            .map_err(|_| "executables store lock poisoned".to_string())?;
        let index = Self::executable_index(&executables, name)?;

        self.authoritative_instances
            .lock()
            .map_err(|_| "authoritative instances lock poisoned".to_string())?
            .insert(executables[index].id(), process.instance_id_owned());

        executables[index].record_started_process(process);

        self.persist_snapshot(&executables)
    }

    fn gate(&self) -> Result<MutexGuard<'_, ()>, String> {
        self.write_gate
            .lock()
            .map_err(|_| "write gate poisoned".to_string())
    }

    fn ensure_writer(&self) -> Result<(), String> {
        if self.authority.can_write() {
            return Ok(());
        }

        Err("supervisor is not the authoritative writer".to_string())
    }

    fn executable_index(executables: &[Executable], name: &str) -> Result<usize, String> {
        executables
            .iter()
            .position(|executable| executable.is_named(name))
            .ok_or_else(|| format!("unknown executable: {name}"))
    }

    fn persist_snapshot(&self, executables: &[Executable]) -> Result<(), String> {
        let runtimes: Vec<ExecutableRuntime> = executables
            .iter()
            .map(Executable::runtime)
            .chain(self.orphans.iter().cloned())
            .collect();

        let encoded = ExecutableRuntime::encode_state(&runtimes)?;

        AtomicFile::write(&self.runtime_path, &encoded)
    }

    fn validate_definitions(executables: &[Executable]) -> Result<(), String> {
        let mut names = HashSet::new();
        let mut default_addresses = HashSet::new();

        for executable in executables {
            executable.validate()?;

            if !names.insert(executable.name()) {
                return Err(format!("duplicate executable: {}", executable.name()));
            }

            if !default_addresses.insert(executable.default_ws_address()) {
                return Err(format!(
                    "duplicate default WebSocket address: {}",
                    executable.default_ws_address()
                ));
            }
        }

        Ok(())
    }

    fn read_runtime_state(
        runtime_path: &Path,
    ) -> Result<HashMap<ExecutableId, ExecutableRuntime>, String> {
        let bytes = match fs::read(runtime_path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(HashMap::new());
            }
            Err(error) => {
                return Err(format!(
                    "failed to read {}: {error}",
                    runtime_path.display()
                ));
            }
        };

        let runtimes = ExecutableRuntime::decode_state(&bytes)
            .map_err(|error| format!("{}: {error}", runtime_path.display()))?;

        Ok(Self::by_id(runtimes))
    }

    fn by_id(runtimes: Vec<ExecutableRuntime>) -> HashMap<ExecutableId, ExecutableRuntime> {
        runtimes
            .into_iter()
            .map(|runtime| (runtime.executable_id(), runtime))
            .collect()
    }

    /// A runtime whose definition no longer exists cannot be tied to a binary
    /// any more, so its process cannot be identified (no `/proc/<pid>/exe`
    /// check) nor stopped. It is kept as long as it holds useful state: a
    /// process whose pid still exists, or an update. Only a runtime with
    /// neither is forgotten.
    fn retain_orphans(runtimes: impl Iterator<Item = ExecutableRuntime>) -> Vec<ExecutableRuntime> {
        let mut orphans = Vec::new();

        for runtime in runtimes {
            let id = runtime.executable_id();

            if let Some(process) = runtime.process() {
                if ManagedProcess::pid_exists(process.pid()) {
                    println!(
                        "[Supervisor] keeping runtime state of undefined executable {id}: pid {} (instance {}, {}) still exists; without its definition it can be neither identified nor stopped",
                        process.pid(),
                        process.instance_id(),
                        process.ws_address(),
                    );

                    orphans.push(runtime);

                    continue;
                }

                println!(
                    "[Supervisor] runtime of undefined executable {id}: pid {} is gone",
                    process.pid()
                );
            }

            if runtime.update().is_some() {
                println!("[Supervisor] keeping update state of undefined executable {id}");

                orphans.push(runtime.without_process());
            } else {
                println!(
                    "[Supervisor] forgetting runtime state of undefined executable {id}: nothing left to keep"
                );
            }
        }

        orphans
    }
}

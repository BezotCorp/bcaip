use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const LOCK_FILE_NAME: &str = "supervisor.authority.lock";
const LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(50);

/// Write authority over the runtime state. A supervisor is the writer only
/// while it holds the exclusive OS lock, which the kernel releases if the
/// process dies, so two writers can never coexist.
pub(crate) struct AuthorityState {
    lock_path: PathBuf,
    lock: Mutex<Option<File>>,
    writer: AtomicBool,
}

impl AuthorityState {
    pub(crate) fn writer(data_directory: &Path) -> Result<Self, String> {
        let state = Self::candidate(data_directory);

        state.acquire(Duration::ZERO)?;

        Ok(state)
    }

    pub(crate) fn candidate(data_directory: &Path) -> Self {
        Self {
            lock_path: data_directory.join(LOCK_FILE_NAME),
            lock: Mutex::new(None),
            writer: AtomicBool::new(false),
        }
    }

    pub(crate) fn can_write(&self) -> bool {
        self.writer.load(Ordering::SeqCst)
    }

    pub(crate) fn acquire(&self, timeout: Duration) -> Result<(), String> {
        let mut lock = self
            .lock
            .lock()
            .map_err(|_| "authority lock poisoned".to_string())?;

        if lock.is_some() {
            return Ok(());
        }

        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&self.lock_path)
            .map_err(|error| format!("failed to open {}: {error}", self.lock_path.display()))?;
        let deadline = Instant::now() + timeout;

        loop {
            match file.try_lock() {
                Ok(()) => break,

                Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                    thread::sleep(LOCK_RETRY_INTERVAL);
                }

                Err(TryLockError::WouldBlock) => {
                    return Err(format!(
                        "another supervisor holds the write authority ({})",
                        self.lock_path.display()
                    ));
                }

                Err(TryLockError::Error(error)) => {
                    return Err(format!(
                        "failed to lock {}: {error}",
                        self.lock_path.display()
                    ));
                }
            }
        }

        *lock = Some(file);
        self.writer.store(true, Ordering::SeqCst);

        Ok(())
    }

    pub(crate) fn release(&self) {
        self.writer.store(false, Ordering::SeqCst);

        if let Ok(mut lock) = self.lock.lock() {
            *lock = None;
        }
    }
}

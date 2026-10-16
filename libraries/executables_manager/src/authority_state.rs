use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const LOCK_FILE_NAME: &str = "bcaip.authority.lock";
const LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(50);

/// Write authority over executable runtime state.
///
/// Authority is held only while this process owns the exclusive OS lock.
/// The kernel releases the lock if the process dies, preventing concurrent
/// writers from holding authority at the same time.
pub struct AuthorityState {
    lock_path: PathBuf,
    lock: Mutex<Option<File>>,
    writer: AtomicBool,
}

impl AuthorityState {
    pub fn writer(data_directory: &Path) -> Result<Self, String> {
        let state = Self::candidate(data_directory);

        state.acquire(Duration::ZERO)?;

        Ok(state)
    }

    pub fn candidate(data_directory: &Path) -> Self {
        Self {
            lock_path: data_directory.join(LOCK_FILE_NAME),
            lock: Mutex::new(None),
            writer: AtomicBool::new(false),
        }
    }

    pub fn can_write(&self) -> bool {
        self.writer.load(Ordering::SeqCst)
    }

    pub fn acquire(&self, timeout: Duration) -> Result<(), String> {
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
                        "another process holds the write authority ({})",
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

    pub fn release(&self) {
        self.writer.store(false, Ordering::SeqCst);

        if let Ok(mut lock) = self.lock.lock() {
            *lock = None;
        }
    }
}

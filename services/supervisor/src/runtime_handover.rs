use crate::executable_store::ExecutableStore;
use crate::handover_journal::HandoverJournal;
use crate::handover_phase::HandoverPhase;
use executables_manager::RuntimeStateBackup;
use std::path::Path;
use std::sync::MutexGuard;
use std::time::Duration;

const RECLAIM_TIMEOUT: Duration = Duration::from_secs(10);

/// Exclusive hold on the runtime state while a candidate is validated.
/// Every store write blocks on the gate, so the file cannot change under the
/// backup; only the candidate (which cannot write) can touch it.
pub(crate) struct RuntimeHandover<'a> {
    store: &'a ExecutableStore,
    _gate: MutexGuard<'a, ()>,
    backup: RuntimeStateBackup,
    journal: HandoverJournal,
    authority_released: bool,
}

impl<'a> RuntimeHandover<'a> {
    pub(crate) fn new(
        store: &'a ExecutableStore,
        gate: MutexGuard<'a, ()>,
        backup: RuntimeStateBackup,
        journal: HandoverJournal,
    ) -> Self {
        Self {
            store,
            _gate: gate,
            backup,
            journal,
            authority_released: false,
        }
    }

    pub(crate) fn backup_path(&self) -> &Path {
        self.backup.copy_path()
    }

    pub(crate) fn is_untouched(&self) -> Result<bool, String> {
        self.backup.matches_current()
    }

    /// Durably records the phase before the authority is given up.
    pub(crate) fn release_authority(&mut self) -> Result<(), String> {
        self.journal.advance(HandoverPhase::Releasing)?;
        self.store.release_authority();
        self.authority_released = true;

        Ok(())
    }

    /// Takes the authority back (if released) and restores the saved state.
    /// The caller must have stopped the candidate first. The write gate stays
    /// held until this handover is dropped, so nothing else can write while
    /// the rollback is incomplete.
    pub(crate) fn rollback(&mut self) -> Result<(), String> {
        // Best effort: a crash from here on is still recovered from the
        // previous phase, which is also rolled back.
        let _ = self.journal.advance(HandoverPhase::RollingBack);

        if self.authority_released {
            self.store.reclaim_authority(RECLAIM_TIMEOUT)?;
            self.authority_released = false;
        }

        self.backup.restore()
    }

    pub(crate) fn reject_update(
        &self,
        name: &str,
        update_id: &str,
        error: String,
    ) -> Result<(), String> {
        self.store.reject_update_while_gated(name, update_id, error)
    }

    /// Rollback finished: the journal is gone and normal operation resumes.
    pub(crate) fn finish_rollback(self) -> Result<(), String> {
        let discarded = HandoverJournal::discard(self.journal.directory());

        if discarded.is_err() {
            self.store.release_authority();
        }

        discarded
    }

    /// Rollback incomplete: nothing may write again in this process. The
    /// journal stays `RollingBack` for the recovery of the next start.
    pub(crate) fn abort_rollback(self) {
        self.store.release_authority();
    }

    /// The candidate owns the authority now (it commits the journal itself);
    /// the old state is never written again.
    pub(crate) fn complete(self) {}
}

use crate::auto_update_diagnostic::AutoUpdateDiagnostic;
use crate::auto_update_phase::AutoUpdatePhase;
use crate::executable_backup::ExecutableBackup;
use crate::executable_store::ExecutableStore;
use crate::handover_journal::HandoverJournal;
use crate::handover_phase::HandoverPhase;
use executables_manager::RuntimeStateBackup;
use std::path::{Path, PathBuf};

/// Startup recovery of an interrupted auto-update. The writer lock is held
/// by the caller, so no other writer and no live candidate can exist. Every
/// step is idempotent and driven by the journal, so a crash while recovering
/// simply resumes the same recovery at the next start.
pub(crate) struct HandoverRecovery {
    data_directory: PathBuf,
    executable_root: PathBuf,
    journal: HandoverJournal,
}

impl HandoverRecovery {
    pub(crate) fn detect(
        data_directory: &Path,
        executable_root: &Path,
    ) -> Result<Option<Self>, String> {
        Ok(HandoverJournal::load(data_directory)?.map(|journal| Self {
            data_directory: data_directory.to_path_buf(),
            executable_root: executable_root.to_path_buf(),
            journal,
        }))
    }

    /// Final step of a successful promotion, done by the promoted supervisor.
    pub(crate) fn confirm_promotion(
        data_directory: &Path,
        executable_root: &Path,
        store: &ExecutableStore,
    ) -> Result<(), String> {
        match Self::detect(data_directory, executable_root)? {
            Some(recovery) if recovery.journal.phase() == HandoverPhase::Committed => {
                recovery.conclude(store).map(|_| ())
            }

            _ => Ok(()),
        }
    }

    /// Must run before the store loads the runtime state.
    pub(crate) fn restore_runtime_state(&mut self) -> Result<(), String> {
        if self.journal.phase() == HandoverPhase::Committed {
            return Ok(());
        }

        self.journal.advance(HandoverPhase::RollingBack)?;

        RuntimeStateBackup::restore_saved(&ExecutableStore::runtime_path(&self.data_directory))
    }

    /// Returns whether the previous binary was restored, in which case the
    /// running process may be the rejected one and must restart.
    pub(crate) fn conclude(self, store: &ExecutableStore) -> Result<bool, String> {
        let update_id = self.journal.update_id();
        let executable = self.journal.executable();

        if self.journal.phase() == HandoverPhase::Committed {
            Self::append(
                &self.data_directory,
                AutoUpdateDiagnostic::new(
                    update_id,
                    executable,
                    AutoUpdatePhase::PromotionConfirmed,
                ),
            );

            HandoverJournal::discard(&self.data_directory)?;

            return Ok(false);
        }

        let mut errors = Vec::new();
        let mut binary_restored = false;

        match store.get(executable)? {
            Some(definition) => match ExecutableBackup::of(&definition.path(&self.executable_root))
            {
                Ok(backup) if backup.is_available() => match backup.restore() {
                    Ok(()) => binary_restored = true,
                    Err(error) => errors.push(error),
                },

                Ok(_) => {}
                Err(error) => errors.push(error),
            },

            None => errors.push(format!("unknown executable {executable}")),
        }

        if let Err(error) = store.reject_update(
            executable,
            update_id,
            "auto-update interrupted by a supervisor crash and rolled back".to_string(),
        ) {
            errors.push(error);
        }

        let diagnostic = AutoUpdateDiagnostic::new(
            update_id,
            executable,
            if errors.is_empty() {
                AutoUpdatePhase::RecoveredRollback
            } else {
                AutoUpdatePhase::RecoveryFailed
            },
        )
        .with_error("handover was interrupted before the promotion was committed".to_string());

        if errors.is_empty() {
            Self::append(&self.data_directory, diagnostic);

            HandoverJournal::discard(&self.data_directory)?;

            return Ok(binary_restored);
        }

        let errors = errors.join("; ");

        Self::append(
            &self.data_directory,
            diagnostic.with_rollback_error(errors.clone()),
        );

        Err(errors)
    }

    fn append(data_directory: &Path, diagnostic: AutoUpdateDiagnostic) {
        if let Err(error) = diagnostic.append(data_directory) {
            println!("[Supervisor] failed to persist recovery diagnostic: {error}");
        }
    }
}

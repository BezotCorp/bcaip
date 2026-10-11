use crate::auto_update_diagnostic::AutoUpdateDiagnostic;
use crate::auto_update_failure::AutoUpdateFailure;
use crate::auto_update_phase::AutoUpdatePhase;
use crate::candidate_client::CandidateClient;
use crate::candidate_process::CandidateProcess;
use crate::executable::Executable;
use crate::executable_backup::ExecutableBackup;
use crate::executable_store::ExecutableStore;
use crate::runtime_handover::RuntimeHandover;
use crate::supervisor_config::SupervisorConfig;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

const VALIDATION_TIMEOUT: Duration = Duration::from_secs(10);
const VALIDATION_INTERVAL: Duration = Duration::from_millis(250);
const TERMINATE_TIMEOUT: Duration = Duration::from_secs(5);

/// Replaces the running supervisor by a freshly updated one. This process
/// stays the only authority until the candidate is validated and promoted;
/// any failure restores the saved state and the previous binary.
pub(crate) struct AutoUpdater {
    store: Arc<ExecutableStore>,
    config: SupervisorConfig,
    supervisor_address: SocketAddr,
    stop: Arc<AtomicBool>,
}

impl AutoUpdater {
    pub(crate) fn new(
        store: Arc<ExecutableStore>,
        config: SupervisorConfig,
        supervisor_address: SocketAddr,
        stop: Arc<AtomicBool>,
    ) -> Self {
        Self {
            store,
            config,
            supervisor_address,
            stop,
        }
    }

    /// Returns whether this supervisor must keep running.
    pub(crate) fn update_supervisor(&self, executable: &Executable) -> bool {
        let update_id = executable
            .committed_update_id()
            .unwrap_or("unknown-update")
            .to_string();
        let candidate = CandidateClient::new(
            self.config.candidate_bind_address(),
            executable.name(),
            &Uuid::now_v7().to_string(),
        );

        let mut handover = match self.store.begin_handover(executable.name(), &update_id) {
            Ok(handover) => handover,

            Err(error) => {
                let failure = AutoUpdateFailure::new(AutoUpdatePhase::BackupFailed, error);

                return self.roll_back(executable, &update_id, &candidate, None, None, failure);
            }
        };

        let mut process = None;

        match self.run_candidate(
            executable,
            &update_id,
            &candidate,
            &mut process,
            &mut handover,
        ) {
            Ok(()) => {
                handover.complete();
                self.persist_diagnostic(
                    AutoUpdateDiagnostic::new(
                        &update_id,
                        executable.name(),
                        AutoUpdatePhase::Promoted,
                    )
                    .with_candidate(
                        process.as_ref().map(CandidateProcess::pid),
                        candidate.instance_id(),
                    ),
                );
                self.stop.store(true, Ordering::SeqCst);

                false
            }

            Err(failure) => self.roll_back(
                executable,
                &update_id,
                &candidate,
                process,
                Some(handover),
                failure,
            ),
        }
    }

    fn run_candidate(
        &self,
        executable: &Executable,
        update_id: &str,
        candidate: &CandidateClient,
        process: &mut Option<CandidateProcess>,
        handover: &mut RuntimeHandover<'_>,
    ) -> Result<(), AutoUpdateFailure> {
        let spawned = CandidateProcess::spawn(
            executable,
            candidate.instance_id(),
            candidate.address(),
            self.supervisor_address,
        )
        .map_err(|error| AutoUpdateFailure::new(AutoUpdatePhase::LaunchFailed, error))?;

        let pid = spawned.pid();

        *process = Some(spawned);

        self.persist_diagnostic(
            AutoUpdateDiagnostic::new(
                update_id,
                executable.name(),
                AutoUpdatePhase::CandidateLaunched,
            )
            .with_candidate(Some(pid), candidate.instance_id())
            .with_runtime_backup(handover.backup_path()),
        );

        self.validate_candidate(executable.name(), update_id, candidate)?;

        match handover.is_untouched() {
            Ok(true) => {}

            Ok(false) => {
                return Err(AutoUpdateFailure::new(
                    AutoUpdatePhase::CorruptionDetected,
                    "candidate modified the protected runtime state before promotion".to_string(),
                ));
            }

            Err(error) => {
                return Err(AutoUpdateFailure::new(
                    AutoUpdatePhase::ValidationFailed,
                    error,
                ));
            }
        }

        handover
            .release_authority()
            .map_err(|error| AutoUpdateFailure::new(AutoUpdatePhase::PromotionFailed, error))?;

        candidate
            .promote()
            .map_err(|error| AutoUpdateFailure::new(AutoUpdatePhase::PromotionFailed, error))
    }

    fn validate_candidate(
        &self,
        executable_name: &str,
        update_id: &str,
        candidate: &CandidateClient,
    ) -> Result<(), AutoUpdateFailure> {
        let fingerprint = self
            .store
            .runtime_fingerprint()
            .map_err(|error| AutoUpdateFailure::new(AutoUpdatePhase::ValidationFailed, error))?;
        let deadline = Instant::now() + VALIDATION_TIMEOUT;
        let mut last_error = String::new();

        while Instant::now() < deadline && !self.stop.load(Ordering::SeqCst) {
            match candidate.report() {
                Ok(report) => {
                    return report
                        .check(
                            executable_name,
                            candidate.instance_id(),
                            update_id,
                            &fingerprint,
                        )
                        .map_err(|error| {
                            AutoUpdateFailure::new(AutoUpdatePhase::ValidationFailed, error)
                        });
                }

                Err(error) => last_error = error,
            }

            thread::sleep(VALIDATION_INTERVAL);
        }

        Err(AutoUpdateFailure::new(
            AutoUpdatePhase::ValidationFailed,
            format!("candidate did not report in time: {last_error}"),
        ))
    }

    /// Stops the candidate, gives the authority and the saved state back,
    /// restores the previous binary and rejects the update so it is not
    /// retried. Returns whether this supervisor must keep running.
    fn roll_back(
        &self,
        executable: &Executable,
        update_id: &str,
        candidate: &CandidateClient,
        process: Option<CandidateProcess>,
        handover: Option<RuntimeHandover<'_>>,
        failure: AutoUpdateFailure,
    ) -> bool {
        let candidate_pid = process.as_ref().map(CandidateProcess::pid);
        let cause = failure.phase();
        let error = failure.into_error();
        let mut diagnostic = AutoUpdateDiagnostic::new(update_id, executable.name(), cause.clone())
            .with_candidate(candidate_pid, candidate.instance_id())
            .with_error(error.clone());

        if let Some(handover) = &handover {
            diagnostic = diagnostic.with_runtime_backup(handover.backup_path());
        }

        self.persist_diagnostic(diagnostic);

        let mut rollback_errors = Vec::new();

        if let Some(process) = process {
            if let Err(stop_error) = process.terminate(TERMINATE_TIMEOUT) {
                rollback_errors.push(stop_error);
            }
        }

        let mut handover = handover;

        if let Some(handover) = handover.as_mut() {
            if let Err(restore_error) = handover.rollback() {
                rollback_errors.push(restore_error);
            }
        }

        if let Err(binary_error) =
            ExecutableBackup::of(&executable.path()).and_then(|backup| backup.restore())
        {
            rollback_errors.push(binary_error);
        }

        let rejection = format!("supervisor candidate rejected ({cause:?}): {error}");
        let rejected = match &handover {
            Some(handover) => handover.reject_update(executable.name(), update_id, rejection),
            None => self
                .store
                .reject_update(executable.name(), update_id, rejection),
        };

        if let Err(reject_error) = rejected {
            rollback_errors.push(reject_error);
        }

        /*
         * The handover (and its write gate) stays alive until here: no write
         * is accepted while the rollback is incomplete. The journal is only
         * discarded once every step succeeded.
         */
        let handover_exists = handover.is_some();
        let mut resumable = rollback_errors.is_empty();

        if let Some(handover) = handover {
            if resumable {
                if let Err(discard_error) = handover.finish_rollback() {
                    rollback_errors.push(discard_error);
                    resumable = false;
                }
            } else {
                handover.abort_rollback();
            }
        }

        let phase = if rollback_errors.is_empty() {
            AutoUpdatePhase::RollbackSucceeded
        } else {
            AutoUpdatePhase::RollbackFailed
        };
        let mut diagnostic = AutoUpdateDiagnostic::new(update_id, executable.name(), phase)
            .with_cause(cause)
            .with_candidate(candidate_pid, candidate.instance_id())
            .with_error(error);

        if !rollback_errors.is_empty() {
            diagnostic = diagnostic.with_rollback_error(rollback_errors.join("; "));
        }

        self.persist_diagnostic(diagnostic);

        if resumable || (!handover_exists && self.store.can_write()) {
            return true;
        }

        /*
         * Incomplete rollback: no normal operation. Exit with the journal in
         * place (still RollingBack): systemd kills the rest of the cgroup,
         * restarts the unit, and the next start runs the idempotent recovery.
         */
        self.stop.store(true, Ordering::SeqCst);

        false
    }

    fn persist_diagnostic(&self, diagnostic: AutoUpdateDiagnostic) {
        if let Err(error) = diagnostic.append(self.config.data_directory()) {
            println!("[Supervisor] failed to persist auto-update diagnostic: {error}");
        }
    }
}

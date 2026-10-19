use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) enum AutoUpdatePhase {
    BackupFailed,
    LaunchFailed,
    CandidateLaunched,
    ValidationFailed,
    CorruptionDetected,
    PromotionFailed,
    Promoted,
    PromotionConfirmed,
    RollbackSucceeded,
    RollbackFailed,
    RecoveredRollback,
    RecoveryFailed,
}

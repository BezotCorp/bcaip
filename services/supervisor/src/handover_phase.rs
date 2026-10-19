use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) enum HandoverPhase {
    /// Runtime state and binary are saved; a candidate may be running.
    Prepared,
    /// The authoritative supervisor is giving up the authority; the
    /// candidate may already hold it.
    Releasing,
    /// The candidate owns the authority and systemd's main process. Final.
    Committed,
    /// The transaction is being undone, live or by startup recovery.
    RollingBack,
}

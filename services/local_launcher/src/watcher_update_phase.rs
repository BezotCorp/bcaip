use serde::Deserialize;

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub(crate) enum WatcherUpdatePhase {
    Pending,
    Committed,
    Failed,
}

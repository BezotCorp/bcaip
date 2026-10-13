use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};

/// The stage of the BCAIP environment lifecycle in which an executable is started.
/// It defines when the executable participates, not which component starts it.
#[derive(Clone, Copy, Debug, Decode, Deserialize, Encode, Eq, PartialEq, Serialize)]
pub enum ExecutableStartPhase {
    /// Started when the BCAIP environment is being set up.
    BcaipEnvironmentSetup,
    /// Started after the BCAIP environment has been set up.
    AfterBcaipEnvironmentSetup,
    /// Started during runtime.
    Runtime,
}

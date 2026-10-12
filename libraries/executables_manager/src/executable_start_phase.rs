use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Decode, Deserialize, Encode, Eq, PartialEq, Serialize)]
pub enum ExecutableStartPhase {
    MachineBoot,
    AfterMachineBoot,
    Runtime,
}

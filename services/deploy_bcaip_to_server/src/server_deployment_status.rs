use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Decode, Deserialize, Encode, Eq, PartialEq, Serialize)]
pub(crate) enum ServerDeploymentStatus {
    NeverBooted,
    Pending,
    Booted,
}

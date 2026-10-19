use bincode_next::{Decode, Encode};
use serde::{Deserialize, Serialize};

/// A responsibility exposed by an executable independently of its name.
#[derive(Clone, Copy, Debug, Decode, Deserialize, Encode, Eq, PartialEq, Serialize)]
pub enum ExecutableRole {
    BcaipAuthorityServer,
}

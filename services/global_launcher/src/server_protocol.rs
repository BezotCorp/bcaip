use bincode_next::{Decode, Encode};
use serde::Deserialize;

#[derive(Encode, Decode, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ServerProtocol {
    Ws,
    Wss,
    Http,
    Https,
}

impl ServerProtocol {
    pub(crate) fn validate(&self) -> Result<(), String> {
        match self {
            Self::Ws | Self::Wss | Self::Http | Self::Https => Ok(()),
        }
    }
}

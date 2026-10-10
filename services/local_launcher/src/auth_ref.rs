use bincode_next::{Decode, Encode};
use serde::Deserialize;
use std::fmt;

#[derive(Encode, Decode, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub(crate) struct AuthRef(String);

impl AuthRef {
    pub(crate) fn validate(&self) -> Result<(), String> {
        let value = self.0.trim();

        if value.is_empty() {
            return Err("remote server auth_ref must not be empty".to_string());
        }

        if value.contains(':') || value.contains('/') || value.contains('\\') {
            return Err(
                "remote server auth_ref must be a credential reference, not a secret or path"
                    .to_string(),
            );
        }

        if !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        {
            return Err("remote server auth_ref contains invalid characters".to_string());
        }

        Ok(())
    }
}

impl fmt::Display for AuthRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

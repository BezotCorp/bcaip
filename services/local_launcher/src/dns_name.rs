use bincode_next::{Decode, Encode};
use serde::Deserialize;
use std::fmt;

#[derive(Encode, Decode, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub(crate) struct DnsName(String);

impl DnsName {
    pub(crate) fn validate(&self) -> Result<(), String> {
        let value = self.0.trim();

        if value.is_empty() {
            return Err("DNS name must not be empty".to_string());
        }

        if value.len() > 253 {
            return Err("DNS name must not exceed 253 characters".to_string());
        }

        if value.starts_with('.') || value.ends_with('.') {
            return Err("DNS name must not start or end with a dot".to_string());
        }

        for label in value.split('.') {
            validate_label(label)?;
        }

        Ok(())
    }
}

impl fmt::Display for DnsName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn validate_label(label: &str) -> Result<(), String> {
    if label.is_empty() {
        return Err("DNS labels must not be empty".to_string());
    }

    if label.len() > 63 {
        return Err("DNS labels must not exceed 63 characters".to_string());
    }

    if label.starts_with('-') || label.ends_with('-') {
        return Err("DNS labels must not start or end with '-'".to_string());
    }

    if !label
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err("DNS labels must contain only ASCII letters, digits, or '-'".to_string());
    }

    Ok(())
}

use std::{
    fs,
    path::{Path, PathBuf},
};

use bincode_next::{Decode, Encode, decode_from_slice, encode_to_vec};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Decode, Deserialize, Encode, Serialize)]
pub(crate) struct ServerDeploymentDescriptor {
    ssh_target: String,
}

impl ServerDeploymentDescriptor {
    pub(crate) fn materialize_and_load(
        ron_path: impl AsRef<Path>,
        binary_path: impl AsRef<Path>,
    ) -> Result<Self, String> {
        let ron_path = ron_path.as_ref();
        let binary_path = binary_path.as_ref();

        let source = fs::read_to_string(ron_path)
            .map_err(|error| format!("failed to read {}: {error}", ron_path.display()))?;

        let descriptor: Self = ron::from_str(&source)
            .map_err(|error| format!("failed to decode {}: {error}", ron_path.display()))?;

        descriptor.validate()?;

        let config = bincode_next::config::standard();

        let encoded = encode_to_vec(&descriptor, config)
            .map_err(|error| format!("failed to encode {}: {error}", ron_path.display()))?;

        let temporary = PathBuf::from(format!("{}.tmp", binary_path.display()));

        fs::write(&temporary, encoded)
            .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;

        fs::rename(&temporary, binary_path).map_err(|error| {
            format!(
                "failed to replace {} from {}: {error}",
                binary_path.display(),
                temporary.display()
            )
        })?;

        let bytes = fs::read(binary_path)
            .map_err(|error| format!("failed to read {}: {error}", binary_path.display()))?;

        let (descriptor, consumed) =
            decode_from_slice::<Self, _>(&bytes, bincode_next::config::standard())
                .map_err(|error| format!("failed to decode {}: {error}", binary_path.display()))?;

        if consumed != bytes.len() {
            return Err(format!("{} contains trailing bytes", binary_path.display()));
        }

        Ok(descriptor)
    }

    pub(crate) fn ssh_target(&self) -> &str {
        &self.ssh_target
    }

    fn validate(&self) -> Result<(), String> {
        if self.ssh_target.trim().is_empty() {
            return Err("ssh_target must not be empty".to_string());
        }

        if self.ssh_target.bytes().any(|byte| byte.is_ascii_control()) {
            return Err("ssh_target contains control characters".to_string());
        }

        Ok(())
    }
}

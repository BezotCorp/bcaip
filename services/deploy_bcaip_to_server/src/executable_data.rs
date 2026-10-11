use std::{
    fs,
    path::{Path, PathBuf},
};

use bincode_next::{decode_from_slice, encode_to_vec};

use crate::executable::Executable;

pub(crate) struct ExecutableData {
    binary_path: PathBuf,
    executables: Vec<Executable>,
}

impl ExecutableData {
    pub(crate) fn load_or_materialize(
        ron_path: impl AsRef<Path>,
        binary_path: impl AsRef<Path>,
    ) -> Result<Self, String> {
        let ron_path = ron_path.as_ref();
        let binary_path = binary_path.as_ref().to_path_buf();

        if !binary_path.exists() {
            let source = fs::read_to_string(ron_path)
                .map_err(|error| format!("failed to read {}: {error}", ron_path.display()))?;

            let executables: Vec<Executable> = ron::from_str(&source)
                .map_err(|error| format!("failed to decode {}: {error}", ron_path.display()))?;

            let encoded = encode_to_vec(&executables, bincode_next::config::standard())
                .map_err(|error| format!("failed to encode executable data: {error}"))?;

            let temporary = binary_path.with_extension("bin.tmp");

            fs::write(&temporary, encoded)
                .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;

            fs::rename(&temporary, &binary_path).map_err(|error| {
                format!(
                    "failed to replace {} from {}: {error}",
                    binary_path.display(),
                    temporary.display()
                )
            })?;
        }

        let bytes = fs::read(&binary_path)
            .map_err(|error| format!("failed to read {}: {error}", binary_path.display()))?;

        let (executables, consumed) =
            decode_from_slice::<Vec<Executable>, _>(&bytes, bincode_next::config::standard())
                .map_err(|error| format!("failed to decode {}: {error}", binary_path.display()))?;

        if consumed != bytes.len() {
            return Err(format!("{} contains trailing bytes", binary_path.display()));
        }

        Ok(Self {
            binary_path,
            executables,
        })
    }

    pub(crate) fn supervisor(&self) -> Result<&Executable, String> {
        self.executables
            .iter()
            .find(|executable| executable.is_named("supervisor"))
            .ok_or_else(|| "supervisor is missing from executable data".to_string())
    }

    pub(crate) fn binary_path(&self) -> &Path {
        &self.binary_path
    }
}

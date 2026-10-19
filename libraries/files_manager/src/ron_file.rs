use std::{fs, path::Path};

use serde::{Serialize, de::DeserializeOwned};

use crate::AtomicFile;

pub struct RonFile;

impl RonFile {
    pub fn read<T>(path: &Path) -> Result<T, String>
    where
        T: DeserializeOwned,
    {
        let bytes = fs::read(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

        Self::decode(path, &bytes)
    }

    pub fn write<T>(path: &Path, value: &T) -> Result<(), String>
    where
        T: Serialize,
    {
        let bytes = Self::encode(path, value)?;

        AtomicFile::write(path, &bytes)
    }

    pub fn encode<T>(path: &Path, value: &T) -> Result<Vec<u8>, String>
    where
        T: Serialize,
    {
        ron::ser::to_string_pretty(value, ron::ser::PrettyConfig::default())
            .map(String::into_bytes)
            .map_err(|error| format!("failed to encode {} as RON: {error}", path.display()))
    }

    pub fn decode<T>(path: &Path, bytes: &[u8]) -> Result<T, String>
    where
        T: DeserializeOwned,
    {
        let content = std::str::from_utf8(bytes)
            .map_err(|error| format!("{} is not valid UTF-8: {error}", path.display()))?;

        ron::from_str(content)
            .map_err(|error| format!("failed to decode {} as RON: {error}", path.display()))
    }
}

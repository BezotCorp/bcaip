use std::{fs, path::Path};

use bincode_next::{Decode, Encode, decode_from_slice, encode_to_vec};

use crate::AtomicFile;

pub struct BinaryFile;

impl BinaryFile {
    pub fn read<T>(path: &Path) -> Result<T, String>
    where
        T: Decode<()>,
    {
        let bytes = fs::read(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

        Self::decode(path, &bytes)
    }

    pub fn write<T>(path: &Path, value: &T) -> Result<(), String>
    where
        T: Encode,
    {
        let bytes = Self::encode(path, value)?;

        AtomicFile::write(path, &bytes)
    }

    pub fn encode<T>(path: &Path, value: &T) -> Result<Vec<u8>, String>
    where
        T: Encode,
    {
        encode_to_vec(value, bincode_next::config::standard())
            .map_err(|error| format!("failed to encode {} as binary: {error}", path.display()))
    }

    pub fn decode<T>(path: &Path, bytes: &[u8]) -> Result<T, String>
    where
        T: Decode<()>,
    {
        let (value, consumed) = decode_from_slice::<T, _>(bytes, bincode_next::config::standard())
            .map_err(|error| format!("failed to decode {} as binary: {error}", path.display()))?;

        if consumed != bytes.len() {
            return Err(format!(
                "{} contains trailing binary data: consumed {} of {} bytes",
                path.display(),
                consumed,
                bytes.len()
            ));
        }

        Ok(value)
    }
}

use std::path::{Path, PathBuf};

use bincode_next::Encode;
use serde::de::DeserializeOwned;

use crate::{BinaryFile, RonFile};

pub struct RonToBinary;

impl RonToBinary {
    pub fn convert<T>(source: &Path, target: &Path) -> Result<(), String>
    where
        T: DeserializeOwned + Encode,
    {
        let value: T = RonFile::read(source)?;

        BinaryFile::write(target, &value)
    }

    pub fn convert_with_extension<T>(
        source: &Path,
        target_extension: &str,
    ) -> Result<PathBuf, String>
    where
        T: DeserializeOwned + Encode,
    {
        let target_extension = target_extension.trim_start_matches('.');

        if target_extension.is_empty()
            || target_extension.contains('/')
            || target_extension.contains('\\')
        {
            return Err(format!("invalid target extension: {target_extension:?}"));
        }

        let target = source.with_extension(target_extension);

        Self::convert::<T>(source, &target)?;

        Ok(target)
    }
}

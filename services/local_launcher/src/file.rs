use crate::extension::Extension;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) struct File {
    directory: PathBuf,
    stem: String,
    extensions: Vec<Extension>,
}

impl File {
    pub(crate) fn new(directory: PathBuf, stem: &str, extensions: Vec<Extension>) -> Self {
        Self {
            directory,
            stem: stem.to_string(),
            extensions,
        }
    }

    pub(crate) fn path_for(&self, extension: Extension) -> Option<PathBuf> {
        self.extensions.contains(&extension).then(|| {
            self.directory
                .join(format!("{}.{}", self.stem, extension.as_str()))
        })
    }

    pub(crate) fn required_path_for(&self, extension: Extension) -> io::Result<PathBuf> {
        self.path_for(extension).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{} extension is not declared for {}",
                    extension.as_str(),
                    self.stem
                ),
            )
        })
    }

    pub(crate) fn read_to_string(&self, extension: Extension) -> io::Result<String> {
        fs::read_to_string(self.required_path_for(extension)?)
    }

    pub(crate) fn read_bytes(&self, extension: Extension) -> io::Result<Vec<u8>> {
        fs::read(self.required_path_for(extension)?)
    }

    pub(crate) fn write_bytes(
        &self,
        extension: Extension,
        content: impl AsRef<[u8]>,
    ) -> io::Result<()> {
        let target = self.required_path_for(extension)?;

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();

        let temp = target.with_extension(format!(
            "{}.{}.{}.tmp",
            extension.as_str(),
            std::process::id(),
            stamp
        ));

        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;

            file.write_all(content.as_ref())?;
            file.sync_all()?;

            fs::rename(&temp, &target)?;
            sync_directory(&self.directory)?;

            Ok(())
        })();

        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }

        result
    }

}

fn sync_directory(path: &Path) -> io::Result<()> {
    fs::File::open(path)?.sync_all()
}

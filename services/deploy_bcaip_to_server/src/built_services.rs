use std::path::PathBuf;

pub(crate) struct BuiltServices {
    supervisor: PathBuf,
    preparer: PathBuf,
}

impl BuiltServices {
    pub(crate) fn new(supervisor: PathBuf, preparer: PathBuf) -> Self {
        Self {
            supervisor,
            preparer,
        }
    }

    pub(crate) fn supervisor(&self) -> &std::path::Path {
        &self.supervisor
    }

    pub(crate) fn preparer(&self) -> &std::path::Path {
        &self.preparer
    }
}

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use executables_manager::ExecutableId;

pub(crate) struct BuiltServices {
    executables: HashMap<ExecutableId, PathBuf>,
}

impl BuiltServices {
    pub(crate) fn new(executables: HashMap<ExecutableId, PathBuf>) -> Self {
        Self { executables }
    }

    pub(crate) fn executable(&self, id: ExecutableId) -> Option<&Path> {
        self.executables.get(&id).map(PathBuf::as_path)
    }
}

use crate::auto_update_phase::AutoUpdatePhase;

pub(crate) struct AutoUpdateFailure {
    phase: AutoUpdatePhase,
    error: String,
}

impl AutoUpdateFailure {
    pub(crate) fn new(phase: AutoUpdatePhase, error: String) -> Self {
        Self { phase, error }
    }

    pub(crate) fn phase(&self) -> AutoUpdatePhase {
        self.phase.clone()
    }

    pub(crate) fn into_error(self) -> String {
        self.error
    }
}

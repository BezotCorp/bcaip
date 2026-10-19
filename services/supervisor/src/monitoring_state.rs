use std::collections::HashMap;

pub(crate) struct MonitoringState {
    failures: HashMap<String, u32>,
    failure_limit: u32,
}

impl MonitoringState {
    pub(crate) fn new(failure_limit: u32) -> Self {
        Self {
            failures: HashMap::new(),
            failure_limit,
        }
    }

    pub(crate) fn reset_executable(&mut self, executable_name: &str) {
        self.failures.remove(executable_name);
    }

    pub(crate) fn executable_failed_enough(&mut self, executable_name: &str) -> bool {
        let failure = self
            .failures
            .entry(executable_name.to_string())
            .or_default();
        *failure = failure.saturating_add(1);

        *failure >= self.failure_limit
    }
}

//! Engine scaffolding for the 2-back task.

#[derive(Debug, Default, Clone)]
pub struct NBackEngine {
    /// Placeholder for the stream length until the actual generator is wired up.
    pub planned_trials: usize,
}

impl NBackEngine {
    pub fn new(planned_trials: usize) -> Self {
        Self { planned_trials }
    }
}

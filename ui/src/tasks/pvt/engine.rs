//! Engine scaffolding for the Psychomotor Vigilance Task (PVT).

#[derive(Debug, Default, Clone)]
pub struct PvtEngine {
    /// Placeholder seed for deterministic schedules while the engine is under construction.
    pub seed: u64,
}

impl PvtEngine {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }
}

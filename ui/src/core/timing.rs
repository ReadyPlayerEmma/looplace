//! High-resolution timing utilities for task engines.

#[derive(Debug, Default, Clone, Copy)]
pub struct TimingSnapshot {
    pub elapsed_ms: f64,
}

impl TimingSnapshot {
    pub fn zero() -> Self {
        Self { elapsed_ms: 0.0 }
    }
}

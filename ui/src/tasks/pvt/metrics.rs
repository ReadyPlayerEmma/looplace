//! Metric definitions for PVT summaries.

#[derive(Debug, Default, Clone)]
pub struct PvtMetrics {
    pub median_rt_ms: f32,
    pub lapses_ge_500ms: u32,
    pub minor_lapses_355_499ms: u32,
    pub false_starts: u32,
}

impl PvtMetrics {
    pub fn empty() -> Self {
        Self::default()
    }
}

//! Metric definitions for 2-back summaries.

#[derive(Debug, Default, Clone)]
pub struct NBackMetrics {
    pub hits: u32,
    pub misses: u32,
    pub false_alarms: u32,
    pub correct_rejections: u32,
}

impl NBackMetrics {
    pub fn empty() -> Self {
        Self::default()
    }
}

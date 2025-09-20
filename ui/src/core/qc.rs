//! Quality control markers for task sessions.

#[derive(Debug, Default, Clone)]
pub struct QualityFlags {
    pub visibility_blur_events: u32,
    pub min_trials_met: bool,
}

impl QualityFlags {
    pub fn pristine() -> Self {
        Self {
            visibility_blur_events: 0,
            min_trials_met: true,
        }
    }
}

//! Quality control markers for task sessions. These flags capture context that helps interpret runs.

use serde::{Deserialize, Serialize};

use super::platform::{platform_string, user_agent_string};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QualityFlags {
    pub visibility_blur_events: u32,
    pub focus_lost_events: u32,
    pub min_trials_met: bool,
    pub device: DeviceSnapshot,
}

impl QualityFlags {
    pub fn pristine() -> Self {
        Self {
            visibility_blur_events: 0,
            focus_lost_events: 0,
            min_trials_met: true,
            device: DeviceSnapshot::capture(),
        }
    }

    pub fn log_visibility_blur(&mut self) {
        self.visibility_blur_events = self.visibility_blur_events.saturating_add(1);
    }

    pub fn log_focus_loss(&mut self) {
        self.focus_lost_events = self.focus_lost_events.saturating_add(1);
    }

    pub fn mark_min_trials(&mut self, met: bool) {
        self.min_trials_met = met;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceSnapshot {
    pub platform: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
}

impl DeviceSnapshot {
    pub fn capture() -> Self {
        Self {
            platform: platform_string(),
            user_agent: user_agent_string(),
        }
    }
}

impl Default for QualityFlags {
    fn default() -> Self {
        Self::pristine()
    }
}

impl Default for DeviceSnapshot {
    fn default() -> Self {
        Self::capture()
    }
}

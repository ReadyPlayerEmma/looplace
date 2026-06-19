//! Full, lossless cognition session records — the structured mirror of the legacy
//! `SummaryRecord`, so the Results UI keeps everything (qc, notes, client) it had.
//!
//! Mostly typed columns; only the genuinely task-variable `metrics` is JSON. The
//! flattened, queryable form of those metrics lives in the `observations` table.

use serde_json::Value;
use time::PrimitiveDateTime;

/// One cognition test session, preserved in full.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionRecord {
    pub id: String,
    pub task: String,
    pub created_at: PrimitiveDateTime,
    pub client_platform: String,
    pub client_tz: String,
    /// Task-specific metrics, preserved verbatim (variable schema → JSON).
    pub metrics: Value,
    pub qc_visibility_blur_events: i64,
    pub qc_focus_lost_events: i64,
    pub qc_min_trials_met: bool,
    pub qc_device_platform: String,
    pub qc_device_user_agent: Option<String>,
    pub notes: Option<String>,
}

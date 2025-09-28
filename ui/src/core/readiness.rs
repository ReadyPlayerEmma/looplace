//! Readiness / cooldown evaluation for cognitive tasks.
//!
//! Rationale
//! ---------
//! We want to *suggest* (not enforce) sensible minimum intervals between
//! consecutive runs so longitudinal data quality isn't eroded by
//! over–frequent sampling (learning effects for n‑back; fatigue rebound
//! for PVT, etc.).
//!
//! Policy (initial)
//! ----------------
//! - 2‑back (`"nback2"`): recommend ≥ 72 h (3 days) between full (main) runs.
//! - PVT (`"pvt"`): recommend ≥ 4 h between runs (multiple daily samples ok).
//!
//! The UI should always allow the user to start a task even if still in a
//! cooldown window; we only surface an advisory indicator.
//!
//! Integration sketch (inside the task view component)
//! ---------------------------------------------------
//! ```ignore
//! let readiness = use_memo(|| {
//!     // Load most recent summary for this task (or pass None)
//!     let last = load_latest_for("pvt"); // Your helper
//!     readiness::evaluate("pvt", last.as_ref())
//! });
//!
//! let status = readiness.status_label();
//! let detail = readiness.detail_message();
//! rsx! {
//!   div { class: format!("task-readiness {}", readiness.css_class()),
//!     span { class: "task-readiness__status", "{status}" }
//!     span { class: "task-readiness__detail", "{detail}" }
//!   }
//! }
//! ```
//!
//! (The styling / component wiring lives in the task view; this module is
//! intentionally pure & platform‑agnostic.)
//!
//! Localization
//! ------------
//! Returned human strings are English. For i18n you can:
//! 1. Use the numeric fields (hours_since, wait_remaining_hours, etc.) and
//!    construct localized sentences via your translation system.
//! 2. Or wrap / fork the helper that emits the strings here.
//!
//! Minimal API
//! -----------
//! - `evaluate(task, last_record)` → `Readiness`
//! - Convenience display helpers:
//!     - `status_label()`
//!     - `detail_message()`
//!     - `css_class()` (for a traffic‑light style)
//!
//! Future extension ideas
//! ----------------------
//! - Per‑user configurable intervals
//! - Distinguish practice vs main runs (already only `main` runs are stored
//!   for n‑back; practice summaries aren't persisted — so current logic is
//!   fine).
//! - Adaptive recommendations (e.g. shorten interval after lapsy PVT run).
//!

use crate::core::storage::SummaryRecord;
use time::{format_description::well_known::Rfc3339, Duration, OffsetDateTime};

/// Output of a readiness evaluation.
#[derive(Debug, Clone)]
pub struct Readiness {
    /// Parsed timestamp of the most recent *persisted* run (UTC) if any.
    pub last_completed: Option<OffsetDateTime>,
    /// Hours elapsed since that run (floating).
    pub hours_since: Option<f64>,
    /// Policy minimum interval (hours).
    pub min_interval_hours: f64,
    /// Whether the recommendation window has elapsed (OK to sample again).
    pub ready: bool,
    /// Hours still remaining until the recommendation window elapses (if not ready).
    pub wait_remaining_hours: Option<f64>,
    /// Convenience: next recommended timestamp (if a last run exists).
    pub next_recommended: Option<OffsetDateTime>,
    /// The task identifier used (echoed for caller convenience).
    pub task: String,
}

impl Readiness {
    /// Short status label suitable for a badge.
    pub fn status_label(&self) -> &'static str {
        if self.ready {
            "Ready"
        } else {
            "Early"
        }
    }

    /// Human detail sentence (English).
    pub fn detail_message(&self) -> String {
        match (self.last_completed, self.hours_since) {
            (None, _) => "No prior runs recorded.".to_string(),
            (Some(_), Some(elapsed)) if self.ready => {
                format!(
                    "Last run {} ago (min interval {}).",
                    human_elapsed(elapsed),
                    human_interval(self.min_interval_hours)
                )
            }
            (Some(_), Some(elapsed)) => {
                let wait = self.wait_remaining_hours.unwrap_or(0.0);
                let next = self
                    .next_recommended
                    .map(|ts| format_rfc3339_compact(ts))
                    .unwrap_or_default();
                format!(
                    "Last run {} ago • wait ~{} (next {next}).",
                    human_elapsed(elapsed),
                    human_elapsed(wait)
                )
            }
            _ => "Unable to compute previous run timing.".to_string(),
        }
    }

    /// A suggested CSS modifier class. Example:
    /// `task-readiness--ready` or `task-readiness--early`
    pub fn css_class(&self) -> &'static str {
        if self.ready {
            "task-readiness--ready"
        } else {
            "task-readiness--early"
        }
    }
}

/// Evaluate readiness for a task given its latest stored summary.
///
/// `task` should match the persisted `SummaryRecord.task` field
/// (e.g. `"pvt"` or `"nback2"`).
pub fn evaluate(task: &str, last: Option<&SummaryRecord>) -> Readiness {
    let min_interval_hours = policy_min_interval_hours(task);
    let last_completed = last.and_then(|r| parse_rfc3339(&r.created_at));
    let now = OffsetDateTime::now_utc();

    let (hours_since, ready, wait_remaining_hours, next_recommended) = match last_completed {
        None => (None, true, None, None),
        Some(ts) => {
            let delta = now - ts;
            let hours = delta.whole_seconds() as f64 / 3600.0;
            let ready = hours >= min_interval_hours;
            let wait_remaining = if ready {
                None
            } else {
                Some((min_interval_hours - hours).max(0.0))
            };
            let next = ts + Duration::seconds((min_interval_hours * 3600.0) as i64);
            (Some(hours), ready, wait_remaining, Some(next))
        }
    };

    Readiness {
        last_completed,
        hours_since,
        min_interval_hours,
        ready,
        wait_remaining_hours,
        next_recommended,
        task: task.to_string(),
    }
}

/// Policy mapping (hard‑coded initial version).
fn policy_min_interval_hours(task: &str) -> f64 {
    match task {
        "nback2" => 72.0, // 3 days
        "pvt" => 4.0,     // 4 hours
        _ => 0.0,         // Unknown task: no restriction
    }
}

/// Parse RFC3339; return None on failure (robust to future format drift).
fn parse_rfc3339(raw: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(raw, &Rfc3339).ok()
}

/// Compact display like `2025-09-28 14:30Z`
fn format_rfc3339_compact(ts: OffsetDateTime) -> String {
    let date = ts.date();
    let time = ts.time();
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}Z",
        date.year(),
        date.month() as u8,
        date.day(),
        time.hour(),
        time.minute()
    )
}

/// Turn elapsed hours into a friendly compact phrase:
/// - < 1 h -> "Xm"
/// - < 48 h -> "Xh"
/// - otherwise -> "Xd Yh" (days floor + remaining hours)
fn human_elapsed(hours: f64) -> String {
    if hours.is_nan() || !hours.is_finite() {
        return "—".into();
    }
    if hours < (1.0 / 60.0) {
        return "<1m".into();
    }
    if hours < 1.0 {
        let mins = (hours * 60.0).round() as i64;
        return format!("{mins}m");
    }
    if hours < 48.0 {
        let whole = hours.round() as i64;
        return format!("{whole}h");
    }
    let days = (hours / 24.0).floor() as i64;
    let rem_hours = (hours - (days as f64 * 24.0)).round() as i64;
    if rem_hours == 0 {
        format!("{days}d")
    } else {
        format!("{days}d {rem_hours}h")
    }
}

/// Human description of a *policy* interval:
///  - 72h -> "3d"
///  - 4h  -> "4h"
fn human_interval(hours: f64) -> String {
    if (hours % 24.0).abs() < f64::EPSILON && hours >= 24.0 {
        format!("{}d", (hours / 24.0).round() as i64)
    } else {
        format!("{hours:.0}h")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::storage::{ClientInfo, SummaryRecord};
    use serde_json::json;

    fn record(task: &str, created_at: OffsetDateTime) -> SummaryRecord {
        SummaryRecord {
            id: "test".into(),
            task: task.into(),
            created_at: created_at.format(&Rfc3339).unwrap(),
            client: ClientInfo {
                platform: "test".into(),
                tz: "UTC".into(),
            },
            metrics: json!({}),
            qc: crate::core::qc::QualityFlags::pristine(),
            notes: None,
        }
    }

    #[test]
    fn ready_when_no_prior() {
        let r = evaluate("pvt", None);
        assert!(r.ready);
        assert!(r.hours_since.is_none());
    }

    #[test]
    fn early_for_recent_nback() {
        let now = OffsetDateTime::now_utc();
        let last = record("nback2", now - Duration::hours(10));
        let r = evaluate("nback2", Some(&last));
        assert!(!r.ready);
        assert!(r.wait_remaining_hours.unwrap() > 0.0);
    }

    #[test]
    fn ready_after_interval() {
        let now = OffsetDateTime::now_utc();
        let last = record("pvt", now - Duration::hours(5));
        let r = evaluate("pvt", Some(&last));
        assert!(r.ready);
    }
}

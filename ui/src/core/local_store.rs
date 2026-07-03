//! Desktop access to the unified local store (`looplace.parquet`) — shared by
//! the cognition write-through and the glucose view.
//!
//! On desktop the store is the **read path** for Results; `summaries.json`
//! remains a parallel write (backup + export compatibility + the web format).
//! Store writes here are best-effort: a failure is logged and never blocks
//! saving a run, because the JSON write has already succeeded.
//!
//! All open→mutate→persist sequences take [`store_lock`] so a cognition save on
//! the UI thread can't race a glucose sync on the device thread (both rewrite
//! the observations file; unsynchronized, the later writer would drop the
//! earlier one's rows).

use std::sync::{Mutex, OnceLock};

use looplace_store::convert::{summary_to_observations, summary_to_session, CognitionSummary};
use looplace_store::{ParquetStore, SessionRecord, Store};

use super::qc::{DeviceSnapshot, QualityFlags};
use super::storage::{data_dir, ClientInfo, SummaryRecord};

/// Process-wide lock serializing all writes to the Parquet store.
pub(crate) fn store_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Open the app's store. Missing files mean an empty store, so first-run is fine.
pub(crate) fn open() -> Result<ParquetStore, String> {
    let dir = data_dir().map_err(|e| format!("data dir unavailable: {e}"))?;
    ParquetStore::open(dir.join("looplace.parquet"))
        .map_err(|e| format!("couldn't open store: {e}"))
}

/// Write-through for a freshly completed session: full record into the sessions
/// table + one observation per numeric metric. Best-effort (logs on failure).
pub fn save_summary(record: &SummaryRecord) {
    if let Err(e) = try_save(record) {
        eprintln!("[store] cognition write-through failed (summaries.json still saved): {e}");
    }
}

fn try_save(record: &SummaryRecord) -> Result<(), String> {
    let summary = to_cognition(record)?;
    let session =
        summary_to_session(&summary).ok_or_else(|| "unparseable created_at".to_string())?;
    let observations = summary_to_observations(&summary);

    let _guard = store_lock().lock().unwrap_or_else(|p| p.into_inner());
    let mut store = open()?;
    store
        .upsert_sessions(std::slice::from_ref(&session))
        .map_err(|e| format!("sessions write failed: {e}"))?;
    store
        .upsert(&observations)
        .map_err(|e| format!("observations write failed: {e}"))?;
    Ok(())
}

/// All stored sessions as view-shaped records (unsorted; caller orders them).
pub fn load_summaries() -> Result<Vec<SummaryRecord>, String> {
    let _guard = store_lock().lock().unwrap_or_else(|p| p.into_inner());
    let store = open()?;
    let sessions = store
        .sessions()
        .map_err(|e| format!("sessions read failed: {e}"))?;
    Ok(sessions.into_iter().map(session_to_summary).collect())
}

/// Delete a session (and its derived observations) from the store. Best-effort.
pub fn delete(id: &str) {
    let _guard = store_lock().lock().unwrap_or_else(|p| p.into_inner());
    match open() {
        Ok(mut store) => {
            if let Err(e) = store.delete_sessions(&[id.to_string()]) {
                eprintln!("[store] session delete failed for {id}: {e}");
            }
        }
        Err(e) => eprintln!("[store] session delete skipped for {id}: {e}"),
    }
}

/// Bridge the UI's `SummaryRecord` to the store's `CognitionSummary` via their
/// shared JSON shape (`CognitionSummary` is the store's mirror of this record).
fn to_cognition(record: &SummaryRecord) -> Result<CognitionSummary, String> {
    let value = serde_json::to_value(record).map_err(|e| format!("serialize: {e}"))?;
    serde_json::from_value(value).map_err(|e| format!("convert: {e}"))
}

/// Reconstruct the view's record from a stored session — the inverse of the
/// migration's `summary_to_session`, so Results keeps its shape.
fn session_to_summary(s: SessionRecord) -> SummaryRecord {
    use time::format_description::well_known::Rfc3339;
    SummaryRecord {
        id: s.id,
        task: s.task,
        created_at: s
            .created_at
            .assume_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into()),
        client: ClientInfo {
            platform: s.client_platform,
            tz: s.client_tz,
        },
        metrics: s.metrics,
        qc: QualityFlags {
            visibility_blur_events: s.qc_visibility_blur_events.max(0) as u32,
            focus_lost_events: s.qc_focus_lost_events.max(0) as u32,
            min_trials_met: s.qc_min_trials_met,
            device: DeviceSnapshot {
                platform: s.qc_device_platform,
                user_agent: s.qc_device_user_agent,
            },
        },
        notes: s.notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A run must survive UI record → store session → UI record unchanged —
    /// this is what makes the store a safe source of truth for Results.
    #[test]
    fn summary_round_trips_through_the_sessions_shape() {
        let original = SummaryRecord {
            id: "pvt-2025-09-21T16:21:54.093347Z-abcd".into(),
            task: "pvt".into(),
            created_at: "2025-09-21T16:21:54.093347Z".into(),
            client: ClientInfo {
                platform: "desktop".into(),
                tz: "America/Denver".into(),
            },
            metrics: serde_json::json!({"median_rt_ms": 312.5, "lapses_ge_500ms": 2}),
            qc: QualityFlags {
                visibility_blur_events: 1,
                focus_lost_events: 0,
                min_trials_met: true,
                device: DeviceSnapshot {
                    platform: "desktop".into(),
                    user_agent: None,
                },
            },
            notes: Some("felt sharp".into()),
        };

        let cognition = to_cognition(&original).unwrap();
        let session = summary_to_session(&cognition).unwrap();
        let restored = session_to_summary(session);
        assert_eq!(restored, original);
    }
}

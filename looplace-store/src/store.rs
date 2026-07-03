//! The storage protocol and an in-memory backend.

use crate::error::Result;
use crate::observation::{Observation, Query};
use crate::session::SessionRecord;

/// The storage protocol. Backends (in-memory, Parquet, later Lance) implement
/// this; callers depend only on the trait.
///
/// Two tables: tidy [`Observation`]s (for correlation across streams) and full
/// [`SessionRecord`]s (lossless cognition sessions, for the Results UI).
pub trait Store {
    /// Idempotently write observations, overwriting any with the same
    /// [`Observation::key`]. Returns the number of *new* rows added.
    fn upsert(&mut self, observations: &[Observation]) -> Result<usize>;

    /// Return observations matching `query`, ordered by timestamp ascending.
    fn query(&self, query: &Query) -> Result<Vec<Observation>>;

    /// Idempotently write session records, overwriting any with the same `id`.
    /// Returns the number of *new* rows added.
    fn upsert_sessions(&mut self, sessions: &[SessionRecord]) -> Result<usize>;

    /// Return all session records, ordered by `created_at` ascending.
    fn sessions(&self) -> Result<Vec<SessionRecord>>;

    /// Delete sessions by id, along with the observations that were derived from
    /// them (matched on `Observation::session_id`). Returns the number of session
    /// rows removed. Unknown ids are ignored.
    fn delete_sessions(&mut self, ids: &[String]) -> Result<usize>;
}

/// In-memory backend — always available, used for tests and as the reference
/// semantics every other backend must match.
#[derive(Debug, Default)]
pub struct MemoryStore {
    rows: Vec<Observation>,
    session_rows: Vec<SessionRecord>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl Store for MemoryStore {
    fn upsert(&mut self, observations: &[Observation]) -> Result<usize> {
        Ok(upsert_into(&mut self.rows, observations))
    }

    fn query(&self, query: &Query) -> Result<Vec<Observation>> {
        Ok(query_rows(&self.rows, query))
    }

    fn upsert_sessions(&mut self, sessions: &[SessionRecord]) -> Result<usize> {
        Ok(upsert_sessions_into(&mut self.session_rows, sessions))
    }

    fn sessions(&self) -> Result<Vec<SessionRecord>> {
        Ok(sorted_sessions(&self.session_rows))
    }

    fn delete_sessions(&mut self, ids: &[String]) -> Result<usize> {
        Ok(delete_sessions_from(
            &mut self.rows,
            &mut self.session_rows,
            ids,
        ))
    }
}

/// Shared upsert semantics over a row vector (overwrite by [`Observation::key`]).
/// Returns the number of *new* rows added. Reused by every in-memory-backed store.
pub(crate) fn upsert_into(rows: &mut Vec<Observation>, observations: &[Observation]) -> usize {
    let mut inserted = 0;
    for obs in observations {
        if let Some(existing) = rows.iter_mut().find(|r| r.key() == obs.key()) {
            *existing = obs.clone();
        } else {
            rows.push(obs.clone());
            inserted += 1;
        }
    }
    inserted
}

/// Shared query semantics: filter then sort by timestamp ascending.
pub(crate) fn query_rows(rows: &[Observation], query: &Query) -> Vec<Observation> {
    let mut out: Vec<Observation> = rows.iter().filter(|o| query.matches(o)).cloned().collect();
    out.sort_by_key(|o| o.timestamp);
    out
}

/// Shared upsert for sessions (overwrite by `id`). Returns new rows added.
pub(crate) fn upsert_sessions_into(
    rows: &mut Vec<SessionRecord>,
    sessions: &[SessionRecord],
) -> usize {
    let mut inserted = 0;
    for session in sessions {
        if let Some(existing) = rows.iter_mut().find(|r| r.id == session.id) {
            *existing = session.clone();
        } else {
            rows.push(session.clone());
            inserted += 1;
        }
    }
    inserted
}

/// Sessions sorted by `created_at` ascending.
pub(crate) fn sorted_sessions(rows: &[SessionRecord]) -> Vec<SessionRecord> {
    let mut out = rows.to_vec();
    out.sort_by_key(|session| session.created_at);
    out
}

/// Shared delete semantics: drop sessions by id and the observations derived
/// from them. Returns the number of session rows removed.
pub(crate) fn delete_sessions_from(
    rows: &mut Vec<Observation>,
    sessions: &mut Vec<SessionRecord>,
    ids: &[String],
) -> usize {
    let before = sessions.len();
    sessions.retain(|s| !ids.contains(&s.id));
    rows.retain(|o| o.session_id.as_ref().map(|id| !ids.contains(id)).unwrap_or(true));
    before - sessions.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn obs(stream: &str, t: time::PrimitiveDateTime, v: f64) -> Observation {
        Observation::new(stream, t, v, "", "dev")
    }

    #[test]
    fn upsert_is_idempotent_on_key() {
        let mut store = MemoryStore::new();
        let a = obs("glucose.mg_dl", datetime!(2026-06-19 08:00:00), 100.0);
        assert_eq!(store.upsert(std::slice::from_ref(&a)).unwrap(), 1);
        // Same key again → overwrite, no new row.
        let mut a2 = a.clone();
        a2.value = 105.0;
        assert_eq!(store.upsert(&[a2]).unwrap(), 0);
        assert_eq!(store.len(), 1);
        assert_eq!(store.query(&Query::default()).unwrap()[0].value, 105.0);
    }

    #[test]
    fn delete_sessions_removes_rows_and_derived_observations() {
        use crate::session::SessionRecord;

        let mut store = MemoryStore::new();
        // Two sessions, each with one derived observation, plus one unrelated
        // glucose row that must survive.
        let session = |id: &str| SessionRecord {
            id: id.into(),
            task: "pvt".into(),
            created_at: datetime!(2026-06-19 08:00:00),
            client_platform: "desktop".into(),
            client_tz: "UTC".into(),
            metrics: serde_json::json!({}),
            qc_visibility_blur_events: 0,
            qc_focus_lost_events: 0,
            qc_min_trials_met: true,
            qc_device_platform: "desktop".into(),
            qc_device_user_agent: None,
            notes: None,
        };
        store
            .upsert_sessions(&[session("pvt-1"), session("pvt-2")])
            .unwrap();
        let mut derived = obs("pvt.median_rt_ms", datetime!(2026-06-19 08:00:00), 300.0);
        derived.session_id = Some("pvt-1".into());
        let mut derived2 = obs("pvt.median_rt_ms", datetime!(2026-06-19 09:00:00), 310.0);
        derived2.session_id = Some("pvt-2".into());
        let unrelated = obs("glucose.mg_dl", datetime!(2026-06-19 08:30:00), 100.0);
        store.upsert(&[derived, derived2, unrelated]).unwrap();

        let removed = store
            .delete_sessions(&["pvt-1".into(), "missing".into()])
            .unwrap();
        assert_eq!(removed, 1);
        assert_eq!(store.sessions().unwrap().len(), 1);
        let remaining = store.query(&Query::default()).unwrap();
        assert_eq!(remaining.len(), 2); // pvt-2's obs + the glucose row
        assert!(remaining.iter().all(|o| o.session_id.as_deref() != Some("pvt-1")));
    }

    #[test]
    fn query_filters_by_stream_and_sorts() {
        let mut store = MemoryStore::new();
        store
            .upsert(&[
                obs("glucose.mg_dl", datetime!(2026-06-19 09:00:00), 110.0),
                obs("glucose.mg_dl", datetime!(2026-06-19 08:00:00), 100.0),
                obs("pvt.median_rt_ms", datetime!(2026-06-19 08:30:00), 300.0),
            ])
            .unwrap();
        let glucose = store.query(&Query::stream("glucose.mg_dl")).unwrap();
        assert_eq!(glucose.len(), 2);
        assert_eq!(glucose[0].value, 100.0); // sorted ascending
        assert_eq!(glucose[1].value, 110.0);
    }
}

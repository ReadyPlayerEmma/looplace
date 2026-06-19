//! The storage protocol and an in-memory backend.

use crate::error::Result;
use crate::observation::{Observation, Query};

/// The storage protocol. Backends (in-memory, Parquet, later Lance) implement
/// this; callers depend only on the trait.
pub trait Store {
    /// Idempotently write observations, overwriting any with the same
    /// [`Observation::key`]. Returns the number of *new* rows added.
    fn upsert(&mut self, observations: &[Observation]) -> Result<usize>;

    /// Return observations matching `query`, ordered by timestamp ascending.
    fn query(&self, query: &Query) -> Result<Vec<Observation>>;
}

/// In-memory backend — always available, used for tests and as the reference
/// semantics every other backend must match.
#[derive(Debug, Default)]
pub struct MemoryStore {
    rows: Vec<Observation>,
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

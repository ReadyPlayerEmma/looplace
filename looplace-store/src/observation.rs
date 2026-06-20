//! The tidy observation row — the unit of storage shared by every stream.

use std::collections::BTreeMap;

use time::PrimitiveDateTime;

/// One scalar measurement on one timeline.
///
/// `stream` is a dotted name (`glucose.mg_dl`, `pvt.median_rt_ms`,
/// `nback2.dprime`). Contextual annotations (food, exercise, reading kind, the
/// device record sequence) live in `tags`.
#[derive(Debug, Clone, PartialEq)]
pub struct Observation {
    pub stream: String,
    pub timestamp: PrimitiveDateTime,
    pub value: f64,
    pub unit: String,
    pub source: String,
    pub session_id: Option<String>,
    pub tags: BTreeMap<String, String>,
}

impl Observation {
    pub fn new(
        stream: impl Into<String>,
        timestamp: PrimitiveDateTime,
        value: f64,
        unit: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            stream: stream.into(),
            timestamp,
            value,
            unit: unit.into(),
            source: source.into(),
            session_id: None,
            tags: BTreeMap::new(),
        }
    }

    /// Idempotency key: one value per `(stream, timestamp, source, kind)`.
    ///
    /// `kind` is the reading-kind tag (`scan`/`sensor`/`blood` for glucose); it
    /// keeps a manual scan and a sensor-trace point that fall in the same minute
    /// from colliding. Streams without a `kind` tag (e.g. cognition) collapse to
    /// `None`, so their dedup behaviour is unchanged. The key is deliberately the
    /// device's *raw* local timestamp — never an offset-adjusted value — so it
    /// stays stable across DST changes and re-syncs.
    pub fn key(&self) -> (&str, PrimitiveDateTime, &str, Option<&str>) {
        (
            &self.stream,
            self.timestamp,
            &self.source,
            self.tags.get("kind").map(String::as_str),
        )
    }
}

/// A simple filter over stored observations.
#[derive(Debug, Clone, Default)]
pub struct Query {
    pub stream: Option<String>,
    pub since: Option<PrimitiveDateTime>,
    pub until: Option<PrimitiveDateTime>,
}

impl Query {
    pub fn stream(stream: impl Into<String>) -> Self {
        Self {
            stream: Some(stream.into()),
            ..Default::default()
        }
    }

    pub fn matches(&self, o: &Observation) -> bool {
        if let Some(s) = &self.stream {
            if &o.stream != s {
                return false;
            }
        }
        if let Some(since) = self.since {
            if o.timestamp < since {
                return false;
            }
        }
        if let Some(until) = self.until {
            if o.timestamp > until {
                return false;
            }
        }
        true
    }
}

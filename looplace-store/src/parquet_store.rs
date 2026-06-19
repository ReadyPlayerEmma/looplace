//! Parquet-file backend for [`Store`] (behind the `parquet-store` feature).
//!
//! Holds an in-memory mirror (so it shares [`MemoryStore`](crate::store::MemoryStore)
//! semantics exactly) and persists the whole table to one Parquet file on each
//! upsert — fine at this data scale, and the file is the portable, DuckDB- and
//! Lance-readable artifact. Writes are atomic (temp file + rename).

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow_array::{
    Array, BooleanArray, Float64Array, Int64Array, RecordBatch, StringArray,
    TimestampMicrosecondArray,
};
use arrow_schema::{DataType, Field, Schema, TimeUnit};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use time::{OffsetDateTime, PrimitiveDateTime};

use crate::error::{Result, StoreError};
use crate::observation::{Observation, Query};
use crate::session::SessionRecord;
use crate::store::{query_rows, sorted_sessions, upsert_into, upsert_sessions_into, Store};

/// A [`Store`] persisted to Parquet: observations at `path`, sessions in a
/// sibling `*.sessions.parquet` file.
pub struct ParquetStore {
    path: PathBuf,
    sessions_path: PathBuf,
    rows: Vec<Observation>,
    sessions: Vec<SessionRecord>,
}

impl ParquetStore {
    /// Open (or create-on-first-write) a store whose observations live at `path`.
    /// The sessions table is the sibling `<path>.sessions.parquet`.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let sessions_path = path.with_extension("sessions.parquet");
        let rows = if path.exists() {
            read_parquet(&path)?
        } else {
            Vec::new()
        };
        let sessions = if sessions_path.exists() {
            read_sessions_parquet(&sessions_path)?
        } else {
            Vec::new()
        };
        Ok(Self {
            path,
            sessions_path,
            rows,
            sessions,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl Store for ParquetStore {
    fn upsert(&mut self, observations: &[Observation]) -> Result<usize> {
        let inserted = upsert_into(&mut self.rows, observations);
        write_parquet(&self.path, &self.rows)?;
        Ok(inserted)
    }

    fn query(&self, query: &Query) -> Result<Vec<Observation>> {
        Ok(query_rows(&self.rows, query))
    }

    fn upsert_sessions(&mut self, sessions: &[SessionRecord]) -> Result<usize> {
        let inserted = upsert_sessions_into(&mut self.sessions, sessions);
        write_sessions_parquet(&self.sessions_path, &self.sessions)?;
        Ok(inserted)
    }

    fn sessions(&self) -> Result<Vec<SessionRecord>> {
        Ok(sorted_sessions(&self.sessions))
    }
}

fn schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("stream", DataType::Utf8, false),
        Field::new(
            "timestamp",
            DataType::Timestamp(TimeUnit::Microsecond, None),
            false,
        ),
        Field::new("value", DataType::Float64, false),
        Field::new("unit", DataType::Utf8, false),
        Field::new("source", DataType::Utf8, false),
        Field::new("session_id", DataType::Utf8, true),
        Field::new("tags_json", DataType::Utf8, false),
    ]))
}

fn pdt_to_micros(t: PrimitiveDateTime) -> i64 {
    (t.assume_utc().unix_timestamp_nanos() / 1_000) as i64
}

fn micros_to_pdt(micros: i64) -> PrimitiveDateTime {
    let odt = OffsetDateTime::from_unix_timestamp_nanos(micros as i128 * 1_000)
        .unwrap_or(OffsetDateTime::UNIX_EPOCH);
    PrimitiveDateTime::new(odt.date(), odt.time())
}

fn write_parquet(path: &Path, rows: &[Observation]) -> Result<()> {
    let schema = schema();

    let stream = StringArray::from_iter_values(rows.iter().map(|r| r.stream.as_str()));
    let timestamp = TimestampMicrosecondArray::from(
        rows.iter().map(|r| pdt_to_micros(r.timestamp)).collect::<Vec<i64>>(),
    );
    let value = Float64Array::from(rows.iter().map(|r| r.value).collect::<Vec<f64>>());
    let unit = StringArray::from_iter_values(rows.iter().map(|r| r.unit.as_str()));
    let source = StringArray::from_iter_values(rows.iter().map(|r| r.source.as_str()));
    let session_id = StringArray::from_iter(rows.iter().map(|r| r.session_id.as_deref()));
    let tags = StringArray::from_iter_values(
        rows.iter()
            .map(|r| serde_json::to_string(&r.tags).unwrap_or_else(|_| "{}".to_string())),
    );

    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(stream),
            Arc::new(timestamp),
            Arc::new(value),
            Arc::new(unit),
            Arc::new(source),
            Arc::new(session_id),
            Arc::new(tags),
        ],
    )
    .map_err(|e| StoreError::Backend(e.to_string()))?;

    write_batch(path, schema, &batch)
}

/// Atomic Parquet write: ensure the parent dir, write a temp file, then rename.
fn write_batch(path: &Path, schema: Arc<Schema>, batch: &RecordBatch) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("store.parquet");
    let tmp = path.with_file_name(format!("{file_name}.tmp"));
    {
        let file = File::create(&tmp)?;
        let mut writer =
            ArrowWriter::try_new(file, schema, None).map_err(|e| StoreError::Backend(e.to_string()))?;
        writer.write(batch).map_err(|e| StoreError::Backend(e.to_string()))?;
        writer.close().map_err(|e| StoreError::Backend(e.to_string()))?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn read_parquet(path: &Path) -> Result<Vec<Observation>> {
    let file = File::open(path)?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| StoreError::Backend(e.to_string()))?
        .build()
        .map_err(|e| StoreError::Backend(e.to_string()))?;

    let mut out = Vec::new();
    for batch in reader {
        let batch = batch.map_err(|e| StoreError::Backend(e.to_string()))?;
        let stream = col_str(&batch, "stream")?;
        let timestamp = batch
            .column_by_name("timestamp")
            .and_then(|c| c.as_any().downcast_ref::<TimestampMicrosecondArray>())
            .ok_or_else(|| StoreError::Backend("timestamp column missing/typed".into()))?;
        let value = batch
            .column_by_name("value")
            .and_then(|c| c.as_any().downcast_ref::<Float64Array>())
            .ok_or_else(|| StoreError::Backend("value column missing/typed".into()))?;
        let unit = col_str(&batch, "unit")?;
        let source = col_str(&batch, "source")?;
        let session_id = col_str(&batch, "session_id")?;
        let tags = col_str(&batch, "tags_json")?;

        for i in 0..batch.num_rows() {
            out.push(Observation {
                stream: stream.value(i).to_string(),
                timestamp: micros_to_pdt(timestamp.value(i)),
                value: value.value(i),
                unit: unit.value(i).to_string(),
                source: source.value(i).to_string(),
                session_id: if session_id.is_null(i) {
                    None
                } else {
                    Some(session_id.value(i).to_string())
                },
                tags: serde_json::from_str(tags.value(i)).unwrap_or_default(),
            });
        }
    }
    Ok(out)
}

fn col_str<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a StringArray> {
    col::<StringArray>(batch, name)
}

/// Downcast a named column to a concrete array type, or a clear error.
fn col<'a, A: 'static>(batch: &'a RecordBatch, name: &str) -> Result<&'a A> {
    batch
        .column_by_name(name)
        .and_then(|c| c.as_any().downcast_ref::<A>())
        .ok_or_else(|| StoreError::Backend(format!("column missing/typed: {name}")))
}

// ---- sessions table -------------------------------------------------------

fn sessions_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("task", DataType::Utf8, false),
        Field::new(
            "created_at",
            DataType::Timestamp(TimeUnit::Microsecond, None),
            false,
        ),
        Field::new("client_platform", DataType::Utf8, false),
        Field::new("client_tz", DataType::Utf8, false),
        Field::new("metrics_json", DataType::Utf8, false),
        Field::new("qc_visibility_blur_events", DataType::Int64, false),
        Field::new("qc_focus_lost_events", DataType::Int64, false),
        Field::new("qc_min_trials_met", DataType::Boolean, false),
        Field::new("qc_device_platform", DataType::Utf8, false),
        Field::new("qc_device_user_agent", DataType::Utf8, true),
        Field::new("notes", DataType::Utf8, true),
    ]))
}

fn write_sessions_parquet(path: &Path, sessions: &[SessionRecord]) -> Result<()> {
    let schema = sessions_schema();

    let id = StringArray::from_iter_values(sessions.iter().map(|s| s.id.as_str()));
    let task = StringArray::from_iter_values(sessions.iter().map(|s| s.task.as_str()));
    let created_at = TimestampMicrosecondArray::from(
        sessions.iter().map(|s| pdt_to_micros(s.created_at)).collect::<Vec<i64>>(),
    );
    let client_platform =
        StringArray::from_iter_values(sessions.iter().map(|s| s.client_platform.as_str()));
    let client_tz = StringArray::from_iter_values(sessions.iter().map(|s| s.client_tz.as_str()));
    let metrics_json = StringArray::from_iter_values(
        sessions
            .iter()
            .map(|s| serde_json::to_string(&s.metrics).unwrap_or_else(|_| "null".to_string())),
    );
    let qc_visibility = Int64Array::from(
        sessions.iter().map(|s| s.qc_visibility_blur_events).collect::<Vec<i64>>(),
    );
    let qc_focus =
        Int64Array::from(sessions.iter().map(|s| s.qc_focus_lost_events).collect::<Vec<i64>>());
    let qc_min = BooleanArray::from(sessions.iter().map(|s| s.qc_min_trials_met).collect::<Vec<bool>>());
    let qc_device =
        StringArray::from_iter_values(sessions.iter().map(|s| s.qc_device_platform.as_str()));
    let qc_user_agent =
        StringArray::from_iter(sessions.iter().map(|s| s.qc_device_user_agent.as_deref()));
    let notes = StringArray::from_iter(sessions.iter().map(|s| s.notes.as_deref()));

    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(id),
            Arc::new(task),
            Arc::new(created_at),
            Arc::new(client_platform),
            Arc::new(client_tz),
            Arc::new(metrics_json),
            Arc::new(qc_visibility),
            Arc::new(qc_focus),
            Arc::new(qc_min),
            Arc::new(qc_device),
            Arc::new(qc_user_agent),
            Arc::new(notes),
        ],
    )
    .map_err(|e| StoreError::Backend(e.to_string()))?;

    write_batch(path, schema, &batch)
}

fn read_sessions_parquet(path: &Path) -> Result<Vec<SessionRecord>> {
    let file = File::open(path)?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| StoreError::Backend(e.to_string()))?
        .build()
        .map_err(|e| StoreError::Backend(e.to_string()))?;

    let mut out = Vec::new();
    for batch in reader {
        let batch = batch.map_err(|e| StoreError::Backend(e.to_string()))?;
        let id = col_str(&batch, "id")?;
        let task = col_str(&batch, "task")?;
        let created_at = col::<TimestampMicrosecondArray>(&batch, "created_at")?;
        let client_platform = col_str(&batch, "client_platform")?;
        let client_tz = col_str(&batch, "client_tz")?;
        let metrics_json = col_str(&batch, "metrics_json")?;
        let qc_visibility = col::<Int64Array>(&batch, "qc_visibility_blur_events")?;
        let qc_focus = col::<Int64Array>(&batch, "qc_focus_lost_events")?;
        let qc_min = col::<BooleanArray>(&batch, "qc_min_trials_met")?;
        let qc_device = col_str(&batch, "qc_device_platform")?;
        let qc_user_agent = col_str(&batch, "qc_device_user_agent")?;
        let notes = col_str(&batch, "notes")?;

        for i in 0..batch.num_rows() {
            out.push(SessionRecord {
                id: id.value(i).to_string(),
                task: task.value(i).to_string(),
                created_at: micros_to_pdt(created_at.value(i)),
                client_platform: client_platform.value(i).to_string(),
                client_tz: client_tz.value(i).to_string(),
                metrics: serde_json::from_str(metrics_json.value(i))
                    .unwrap_or(serde_json::Value::Null),
                qc_visibility_blur_events: qc_visibility.value(i),
                qc_focus_lost_events: qc_focus.value(i),
                qc_min_trials_met: qc_min.value(i),
                qc_device_platform: qc_device.value(i).to_string(),
                qc_device_user_agent: nullable(qc_user_agent, i),
                notes: nullable(notes, i),
            });
        }
    }
    Ok(out)
}

fn nullable(arr: &StringArray, i: usize) -> Option<String> {
    if arr.is_null(i) {
        None
    } else {
        Some(arr.value(i).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("looplace_store_pq");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}.parquet"));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn round_trips_through_a_parquet_file() {
        let path = temp_path("roundtrip");

        let mut glucose = Observation::new(
            "glucose.mg_dl",
            datetime!(2026-06-19 09:11:30),
            94.0,
            "mg/dL",
            "MPGF176-T4167",
        );
        glucose.tags.insert("food_carbs_grams".into(), "15".into());
        glucose.tags.insert("exercise".into(), "true".into());

        let mut pvt = Observation::new(
            "pvt.median_rt_ms",
            datetime!(2026-06-19 08:00:00),
            312.5,
            "ms",
            "looplace",
        );
        pvt.session_id = Some("pvt-1".into());

        {
            let mut store = ParquetStore::open(&path).unwrap();
            assert_eq!(store.upsert(&[glucose.clone(), pvt.clone()]).unwrap(), 2);
        }

        // Reopen from disk: data survives, tags + session_id + timestamp intact.
        let store = ParquetStore::open(&path).unwrap();
        assert_eq!(store.len(), 2);
        assert_eq!(store.query(&Query::stream("glucose.mg_dl")).unwrap()[0], glucose);
        assert_eq!(store.query(&Query::stream("pvt.median_rt_ms")).unwrap()[0], pvt);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn upsert_persists_and_is_idempotent() {
        let path = temp_path("idempotent");
        let observation = Observation::new(
            "glucose.mg_dl",
            datetime!(2026-06-19 09:00:00),
            100.0,
            "mg/dL",
            "dev",
        );

        let mut store = ParquetStore::open(&path).unwrap();
        assert_eq!(store.upsert(std::slice::from_ref(&observation)).unwrap(), 1);

        let mut updated = observation.clone();
        updated.value = 105.0;
        assert_eq!(store.upsert(&[updated]).unwrap(), 0); // same key → overwrite

        let reopened = ParquetStore::open(&path).unwrap();
        let rows = reopened.query(&Query::default()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].value, 105.0);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn sessions_round_trip_losslessly_through_parquet() {
        let path = temp_path("sessions");
        let sessions_file = path.with_extension("sessions.parquet");
        let _ = std::fs::remove_file(&sessions_file);

        let desktop = SessionRecord {
            id: "pvt-1".into(),
            task: "pvt".into(),
            created_at: datetime!(2025-09-21 16:21:54.093347),
            client_platform: "desktop".into(),
            client_tz: "America/Chicago".into(),
            metrics: serde_json::json!({"median_rt_ms": 312.5, "lapses_ge_500ms": 2}),
            qc_visibility_blur_events: 0,
            qc_focus_lost_events: 1,
            qc_min_trials_met: true,
            qc_device_platform: "desktop".into(),
            qc_device_user_agent: None,
            notes: Some("felt sharp".into()),
        };
        let web = SessionRecord {
            id: "nback2-1".into(),
            task: "nback2".into(),
            created_at: datetime!(2025-09-25 20:00:00),
            client_platform: "web".into(),
            client_tz: "UTC".into(),
            metrics: serde_json::json!({"dprime": 1.8}),
            qc_visibility_blur_events: 2,
            qc_focus_lost_events: 0,
            qc_min_trials_met: false,
            qc_device_platform: "web".into(),
            qc_device_user_agent: Some("Mozilla/5.0".into()),
            notes: None,
        };

        {
            let mut store = ParquetStore::open(&path).unwrap();
            assert_eq!(store.upsert_sessions(&[desktop.clone(), web.clone()]).unwrap(), 2);
        }

        // Reopen: metrics JSON, qc fields, and nullable user_agent/notes survive.
        let store = ParquetStore::open(&path).unwrap();
        let sessions = store.sessions().unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0], desktop); // sorted by created_at
        assert_eq!(sessions[1], web);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&sessions_file);
    }
}

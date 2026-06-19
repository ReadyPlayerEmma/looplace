//! Parquet-file backend for [`Store`] (behind the `parquet-store` feature).
//!
//! Holds an in-memory mirror (so it shares [`MemoryStore`](crate::store::MemoryStore)
//! semantics exactly) and persists the whole table to one Parquet file on each
//! upsert — fine at this data scale, and the file is the portable, DuckDB- and
//! Lance-readable artifact. Writes are atomic (temp file + rename).

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow_array::{Array, Float64Array, RecordBatch, StringArray, TimestampMicrosecondArray};
use arrow_schema::{DataType, Field, Schema, TimeUnit};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use time::{OffsetDateTime, PrimitiveDateTime};

use crate::error::{Result, StoreError};
use crate::observation::{Observation, Query};
use crate::store::{query_rows, upsert_into, Store};

/// A [`Store`] persisted to a single Parquet file.
pub struct ParquetStore {
    path: PathBuf,
    rows: Vec<Observation>,
}

impl ParquetStore {
    /// Open (or create-on-first-write) a store at `path`, loading any existing rows.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let rows = if path.exists() {
            read_parquet(&path)?
        } else {
            Vec::new()
        };
        Ok(Self { path, rows })
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

    // Atomic write: temp file in the same directory, then rename.
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("store.parquet");
    let tmp = path.with_file_name(format!("{file_name}.tmp"));
    {
        let file = File::create(&tmp)?;
        let mut writer =
            ArrowWriter::try_new(file, schema, None).map_err(|e| StoreError::Backend(e.to_string()))?;
        writer.write(&batch).map_err(|e| StoreError::Backend(e.to_string()))?;
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
    batch
        .column_by_name(name)
        .and_then(|c| c.as_any().downcast_ref::<StringArray>())
        .ok_or_else(|| StoreError::Backend(format!("column missing/typed: {name}")))
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
}

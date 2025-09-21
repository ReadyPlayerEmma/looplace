//! Local persistence helpers for summaries and settings.

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::platform::{platform_string, timezone_string};
use super::qc::QualityFlags;

#[cfg(target_arch = "wasm32")]
const STORAGE_KEY: &str = "looplace_summaries";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SummaryRecord {
    pub id: String,
    pub task: String,
    pub created_at: String,
    pub client: ClientInfo,
    pub metrics: serde_json::Value,
    pub qc: QualityFlags,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientInfo {
    pub platform: String,
    pub tz: String,
}

impl SummaryRecord {
    pub fn new<T: Into<String>>(task: T, metrics: serde_json::Value, qc: QualityFlags) -> Self {
        let task_string = task.into();
        Self {
            id: new_summary_id(&task_string),
            task: task_string,
            created_at: current_timestamp_iso(),
            client: ClientInfo::current(),
            metrics,
            qc,
            notes: None,
        }
    }
}

impl ClientInfo {
    pub fn current() -> Self {
        Self {
            platform: platform_string(),
            tz: timezone_string(),
        }
    }
}

#[derive(Debug)]
pub enum StorageError {
    Serialization(serde_json::Error),
    LocalUnavailable,
    WriteFailed,
    ReadFailed,
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for StorageError {}

pub fn load_summaries() -> Result<Vec<SummaryRecord>, StorageError> {
    #[cfg(target_arch = "wasm32")]
    {
        let storage = local_storage().ok_or(StorageError::LocalUnavailable)?;
        match storage.get_item(STORAGE_KEY) {
            Ok(Some(raw)) => serde_json::from_str(&raw).map_err(StorageError::Serialization),
            Ok(None) => Ok(Vec::new()),
            Err(_) => Err(StorageError::ReadFailed),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        Ok(MEMORY_DB.lock().expect("storage lock poisoned").clone())
    }
}

pub fn append_summary(summary: &SummaryRecord) -> Result<(), StorageError> {
    let mut records = load_summaries()?;
    records.push(summary.clone());
    save_all(&records)
}

pub fn save_all(records: &[SummaryRecord]) -> Result<(), StorageError> {
    #[cfg(target_arch = "wasm32")]
    {
        let storage = local_storage().ok_or(StorageError::LocalUnavailable)?;
        let serialized = serde_json::to_string(&records).map_err(StorageError::Serialization)?;
        storage
            .set_item(STORAGE_KEY, &serialized)
            .map_err(|_| StorageError::WriteFailed)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut guard = MEMORY_DB.lock().expect("storage lock poisoned");
        *guard = records.to_vec();
        Ok(())
    }
}

fn new_summary_id(task: &str) -> String {
    format!("{task}-{}-{}", current_timestamp_iso(), Uuid::new_v4())
}

fn current_timestamp_iso() -> String {
    use time::{format_description::well_known::Rfc3339, OffsetDateTime};

    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

#[cfg(target_arch = "wasm32")]
fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
}

#[cfg(not(target_arch = "wasm32"))]
use once_cell::sync::Lazy;

#[cfg(not(target_arch = "wasm32"))]
use std::sync::Mutex;

#[cfg(not(target_arch = "wasm32"))]
static MEMORY_DB: Lazy<Mutex<Vec<SummaryRecord>>> = Lazy::new(|| Mutex::new(Vec::new()));

pub fn clear_all() -> Result<(), StorageError> {
    #[cfg(target_arch = "wasm32")]
    {
        let storage = local_storage().ok_or(StorageError::LocalUnavailable)?;
        storage
            .set_item(STORAGE_KEY, "[]")
            .map_err(|_| StorageError::WriteFailed)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        MEMORY_DB.lock().expect("storage lock poisoned").clear();
        Ok(())
    }
}

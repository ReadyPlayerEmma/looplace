//! Store error type (std-only, mirroring the pattern used across the workspace).

use std::fmt;

pub type Result<T> = std::result::Result<T, StoreError>;

#[derive(Debug)]
pub enum StoreError {
    Io(String),
    Parse(String),
    Backend(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Io(m) => write!(f, "io error: {m}"),
            StoreError::Parse(m) => write!(f, "parse error: {m}"),
            StoreError::Backend(m) => write!(f, "backend error: {m}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        StoreError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        StoreError::Parse(e.to_string())
    }
}

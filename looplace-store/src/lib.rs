//! Local health + cognition data store for Looplace (native-only).
//!
//! A single [`Store`] trait is the storage protocol; backends are swappable
//! implementations. Today: an in-memory backend (always available) and a Parquet
//! backend (behind `parquet-store`). A Lance backend can be added later as
//! another impl without touching callers — Arrow/Parquet interchange keeps that
//! swap cheap.
//!
//! All streams are stored in a uniform tidy shape ([`Observation`]): one row per
//! scalar measurement, so glucose, cognition metrics, and future Apple Health
//! data share a timeline and join trivially.
//!
//! Time basis: timestamps are [`time::PrimitiveDateTime`] in the *source's local
//! wall-clock* (what the Libre reader reports). Cross-source timezone unification
//! (e.g. cognition `created_at` is UTC) is a known TODO before correlation.

pub mod convert;
pub mod error;
pub mod migrate;
pub mod observation;
pub mod session;
pub mod store;

#[cfg(feature = "parquet-store")]
pub mod parquet_store;

pub use error::{Result, StoreError};
pub use observation::{Observation, Query};
pub use session::SessionRecord;
pub use store::{MemoryStore, Store};

#[cfg(feature = "parquet-store")]
pub use parquet_store::ParquetStore;

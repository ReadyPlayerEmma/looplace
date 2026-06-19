//! `looplace-libre` — native-Rust FreeStyle Libre 2 driver + local health ingest.
//!
//! This crate is the native data layer for Looplace's health pipeline. It is
//! deliberately kept out of the shared `ui/` crate, which must keep compiling to
//! `wasm32`; everything here (USB transport in Phase 1, DuckDB store in Phase 3)
//! is native-only.
//!
//! ## Status
//!
//! - [`crypto`] — **done.** Dependency-free port of the Libre 2 session
//!   cryptography (Speck 64/128, CTR stream, Speck-CMAC, CMAC-KDF), verified
//!   byte-for-byte against the canonical Python reference. See
//!   `examples/selfcheck.rs` for the reference vectors.
//! - [`transport`] — HID byte channel. [`transport::HidTransport`] trait, an
//!   in-memory [`transport::ReplayTransport`] for offline validation, and the
//!   `hidapi`-backed `HidApiTransport` behind the `transport` feature.
//! - [`session`] — FreeStyle protocol: framing, encrypted handshake, and
//!   command/response, ported from `_session.py` and validated against the
//!   reference Python via replay (see its tests). Encrypted devices need the
//!   `libre2-keys` feature.
//! - [`records`] — record parsing + the [`records::Reading`] model
//!   (`$history?` CGM trace, `$arresult?` scans/blood/ketone/annotations/clock).
//! - [`device`] — high-level [`device::LibreDevice`]: connect, identity/units,
//!   and `read_all()`.
//! - [`error`] — shared error type.
//!
//! ## Licensing
//!
//! The crate as a whole is MPL-2.0, but [`crypto`] is a clean port of
//! Apache-2.0 upstream code and carries its own SPDX header — preserve it. The
//! four Libre 2 authorization-key constants (the licensing "gray artifact") do
//! **not** live here; they will live in a separate optional keys crate, excluded
//! from any published build by default.

pub mod crypto;
pub mod device;
pub mod error;
pub mod records;
pub mod session;
pub mod transport;

pub use device::{LibreDevice, Unit};
pub use error::{LibreError, Result};
pub use records::{Annotations, GlucoseSource, Reading};
pub use session::Session;

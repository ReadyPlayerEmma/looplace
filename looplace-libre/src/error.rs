//! Crate error type. Std-only, mirroring the `StorageError` pattern in `ui/`
//! (no `thiserror` dependency at this stage).

use std::fmt;

pub type Result<T> = std::result::Result<T, LibreError>;

#[derive(Debug)]
pub enum LibreError {
    /// No FreeStyle Libre 2 reader matched the expected USB VID/PID.
    DeviceNotFound,
    /// A HID transport-level read/write failed.
    Transport(String),
    /// The session handshake (auth / key derivation) failed.
    Handshake(String),
    /// A device response could not be parsed into the expected record shape.
    Parse(String),
    /// Functionality that is not yet implemented in the current phase.
    Unimplemented(&'static str),
}

impl fmt::Display for LibreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LibreError::DeviceNotFound => write!(f, "no FreeStyle Libre 2 reader found"),
            LibreError::Transport(m) => write!(f, "HID transport error: {m}"),
            LibreError::Handshake(m) => write!(f, "session handshake error: {m}"),
            LibreError::Parse(m) => write!(f, "record parse error: {m}"),
            LibreError::Unimplemented(what) => write!(f, "not implemented yet: {what}"),
        }
    }
}

impl std::error::Error for LibreError {}

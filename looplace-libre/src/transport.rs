//! HID transport for the FreeStyle Libre 2 reader.
//!
//! Ported from freestyle-hid's `_hidwrapper.py` (Apache-2.0). The [`HidTransport`]
//! trait abstracts the raw byte channel so the session ([`crate::session`]) can run
//! against either a physical reader ([`HidApiTransport`], behind the `transport`
//! feature) or recorded frames ([`ReplayTransport`]) for offline validation.

use std::collections::VecDeque;

use crate::error::{LibreError, Result};

/// USB vendor id for Abbott Diabetes Care (FreeStyle readers).
pub const USB_VENDOR_ID: u16 = 0x1A61;
/// USB product id for the FreeStyle Libre 2 reader (encrypted protocol).
pub const USB_PRODUCT_ID_LIBRE2: u16 = 0x3950;
/// USB product id for the original FreeStyle Libre reader (unencrypted).
pub const USB_PRODUCT_ID_LIBRE: u16 = 0x3650;

/// HID report size, in bytes, used by the reader's framing.
pub const REPORT_LENGTH: usize = 64;

/// A raw HID byte channel. Implementations move already-framed reports
/// (a report-id byte followed by up to 64 content bytes) to and from the device.
pub trait HidTransport {
    /// Write one outbound report (report-id byte included).
    fn write(&mut self, data: &[u8]) -> Result<()>;
    /// Read one inbound report (up to [`REPORT_LENGTH`] bytes).
    fn read(&mut self) -> Result<Vec<u8>>;
}

/// In-memory transport for offline capture-and-replay validation and unit tests.
///
/// Queue the device's responses with [`ReplayTransport::new`]; inspect what the
/// session actually wrote with [`ReplayTransport::written`] to diff against a
/// captured oracle.
pub struct ReplayTransport {
    reads: VecDeque<Vec<u8>>,
    written: Vec<Vec<u8>>,
}

impl ReplayTransport {
    /// Build a replay transport from the ordered device responses to hand back.
    pub fn new(reads: impl IntoIterator<Item = Vec<u8>>) -> Self {
        Self {
            reads: reads.into_iter().collect(),
            written: Vec::new(),
        }
    }

    /// The frames the session wrote, in order (each includes the report-id byte).
    pub fn written(&self) -> &[Vec<u8>] {
        &self.written
    }
}

impl HidTransport for ReplayTransport {
    fn write(&mut self, data: &[u8]) -> Result<()> {
        self.written.push(data.to_vec());
        Ok(())
    }

    fn read(&mut self) -> Result<Vec<u8>> {
        self.reads
            .pop_front()
            .ok_or_else(|| LibreError::Transport("replay transport exhausted".into()))
    }
}

#[cfg(feature = "transport")]
pub use hidapi_transport::HidApiTransport;

#[cfg(feature = "transport")]
mod hidapi_transport {
    use super::*;
    use hidapi::{HidApi, HidDevice};
    use std::sync::{Mutex, OnceLock};

    /// hidapi must be initialized once per process. Re-creating it (a `hid_exit`
    /// on drop, then a later `hid_init`) is fragile on macOS, and — critically —
    /// an open [`HidDevice`] does *not* keep the context alive, so dropping the
    /// `HidApi` while a device is still open is a use-after-free that corrupts the
    /// global HID state (it "works" once, then crashes on the next open). So we
    /// hold a single `HidApi` for the whole program and open every device from it.
    fn shared_api() -> Result<&'static Mutex<HidApi>> {
        static API: OnceLock<Mutex<HidApi>> = OnceLock::new();
        if let Some(api) = API.get() {
            return Ok(api);
        }
        // `OnceLock::get_or_init` can't carry the fallible init, so construct
        // first; a lost init race just drops a redundant context (opens are
        // serialized in practice, so this is effectively never hit).
        let api = HidApi::new().map_err(|e| LibreError::Transport(e.to_string()))?;
        Ok(API.get_or_init(|| Mutex::new(api)))
    }

    /// Transport backed by a physical reader via the `hidapi` crate. The device
    /// handle is opened from the process-wide [`shared_api`], which stays alive,
    /// so the handle remains valid for as long as this transport lives.
    pub struct HidApiTransport {
        device: HidDevice,
    }

    impl HidApiTransport {
        /// Open the first connected device matching `vendor_id`/`product_id`.
        pub fn open(vendor_id: u16, product_id: u16) -> Result<Self> {
            let mut api = shared_api()?
                .lock()
                .map_err(|_| LibreError::Transport("hidapi context poisoned".into()))?;
            // Re-scan so a reader plugged in (or replugged) after first init is seen.
            api.refresh_devices()
                .map_err(|e| LibreError::Transport(e.to_string()))?;
            let device = api
                .open(vendor_id, product_id)
                .map_err(|_| LibreError::DeviceNotFound)?;
            Ok(Self { device })
        }

        /// Convenience opener for the FreeStyle Libre 2 reader.
        pub fn open_libre2() -> Result<Self> {
            Self::open(USB_VENDOR_ID, USB_PRODUCT_ID_LIBRE2)
        }
    }

    impl HidTransport for HidApiTransport {
        fn write(&mut self, data: &[u8]) -> Result<()> {
            self.device
                .write(data)
                .map_err(|e| LibreError::Transport(e.to_string()))?;
            Ok(())
        }

        fn read(&mut self) -> Result<Vec<u8>> {
            // Blocking read of a single input report. A safety timeout surfaces a
            // silent/stuck reader as an error rather than hanging forever; a valid
            // exchange always replies promptly.
            let mut buf = vec![0u8; REPORT_LENGTH];
            let n = self
                .device
                .read_timeout(&mut buf, 3000)
                .map_err(|e| LibreError::Transport(e.to_string()))?;
            if n == 0 {
                return Err(LibreError::Transport(
                    "HID read timed out (no response from reader)".into(),
                ));
            }
            buf.truncate(n);
            Ok(buf)
        }
    }
}

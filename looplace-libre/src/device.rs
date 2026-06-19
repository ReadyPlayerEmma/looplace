//! High-level FreeStyle Libre device API over a [`Session`].
//!
//! Mirrors glucometerutils' `LibreDevice`: connect, read identity/units, and pull
//! the full reading set (`$history?` CGM trace + `$arresult?` events).

use crate::error::{LibreError, Result};
use crate::records::{parse_arresult_record, parse_history_record, Reading};
use crate::session::Session;
use crate::transport::HidTransport;

/// The glucose unit the device is configured to display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    MmolL,
    MgDl,
}

/// A FreeStyle Libre / Libre 2 device. Libre uses encrypted text framing 0x60/0x60.
pub struct LibreDevice<T: HidTransport> {
    session: Session<T>,
}

impl<T: HidTransport> LibreDevice<T> {
    /// Wrap a transport as an encrypted Libre 2 device.
    pub fn new(transport: T) -> Self {
        Self {
            session: Session::new(transport, 0x60, 0x60, true),
        }
    }

    /// Wrap a pre-built session (e.g. an unencrypted Libre, or for testing).
    pub fn from_session(session: Session<T>) -> Self {
        Self { session }
    }

    /// Access the underlying session.
    pub fn session_mut(&mut self) -> &mut Session<T> {
        &mut self.session
    }

    /// Open the connection with a caller-supplied host nonce.
    pub fn connect_with_nonce(&mut self, host_nonce: [u8; 8]) -> Result<()> {
        self.session.connect_with_nonce(host_nonce)
    }

    /// Open the connection with an OS-random host nonce (real-device path).
    #[cfg(feature = "transport")]
    pub fn connect(&mut self) -> Result<()> {
        self.session.connect()
    }

    /// Device serial number (Libre uses `$sn?`, not the base `$serlnum?`).
    pub fn serial_number(&mut self) -> Result<String> {
        Ok(self.session.send_text_command(b"$sn?")?.trim().to_string())
    }

    /// Device software version.
    pub fn software_version(&mut self) -> Result<String> {
        Ok(self.session.send_text_command(b"$swver?")?.trim().to_string())
    }

    /// Configured glucose unit (`$uom?`: 0 = mmol/L, 1 = mg/dL).
    pub fn glucose_unit(&mut self) -> Result<Unit> {
        match self.session.send_text_command(b"$uom?")?.trim() {
            "0" => Ok(Unit::MmolL),
            "1" => Ok(Unit::MgDl),
            other => Err(LibreError::Parse(format!("invalid glucose unit: {other}"))),
        }
    }

    /// Pull every reading: the CGM sensor trace then the explicit events.
    /// Glucose values are mg/dL (device-internal); ketones are mmol/L.
    pub fn read_all(&mut self) -> Result<Vec<Reading>> {
        let mut readings = Vec::new();
        for record in self.session.query_multirecord(b"$history?")? {
            if let Some(reading) = parse_history_record(&record) {
                readings.push(reading);
            }
        }
        for record in self.session.query_multirecord(b"$arresult?")? {
            if let Some(reading) = parse_arresult_record(&record) {
                readings.push(reading);
            }
        }
        Ok(readings)
    }
}

#[cfg(feature = "transport")]
impl LibreDevice<crate::transport::HidApiTransport> {
    /// Open the first connected FreeStyle Libre 2 reader over USB.
    pub fn open_libre2() -> Result<Self> {
        Ok(Self::new(crate::transport::HidApiTransport::open_libre2()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::records::GlucoseSource;
    use crate::transport::ReplayTransport;

    /// Encode a multirecord text reply (records + count/checksum + outer checksum)
    /// as a sequence of 0x60 HID input reports, the way the device would.
    fn multirecord_reports(records: &[&str]) -> Vec<Vec<u8>> {
        let mut records_raw = String::new();
        for r in records {
            records_raw.push_str(r);
            records_raw.push_str("\r\n");
        }
        let inner: u32 = records_raw.bytes().map(|b| b as u32).sum();
        let message = format!("{records_raw}{},{inner:08X}\r\n", records.len());
        let outer: u32 = message.bytes().map(|b| b as u32).sum();
        let full = format!("{message}CKSM:{outer:08X}\r\nCMD OK\r\n").into_bytes();

        full.chunks(62)
            .map(|chunk| {
                let mut content = vec![0x60u8, chunk.len() as u8];
                content.extend_from_slice(chunk);
                content.resize(64, 0);
                content
            })
            .collect()
    }

    /// A single-line text reply ("Log Empty") as one report.
    fn text_reply_reports(body: &str) -> Vec<Vec<u8>> {
        let outer: u32 = body.bytes().map(|b| b as u32).sum();
        let full = format!("{body}CKSM:{outer:08X}\r\nCMD OK\r\n").into_bytes();
        full.chunks(62)
            .map(|chunk| {
                let mut content = vec![0x60u8, chunk.len() as u8];
                content.extend_from_slice(chunk);
                content.resize(64, 0);
                content
            })
            .collect()
    }

    #[test]
    fn read_all_parses_history_and_skips_empty_arresult() {
        // Two CGM history records + an empty arresult log.
        let history = vec![
            "12,0,6,19,26,8,30,0,0,0,0,0,0,101,0,0",
            "12,0,6,19,26,8,45,0,0,0,0,0,0,109,0,0",
        ];
        let mut reports = multirecord_reports(&history);
        reports.extend(text_reply_reports("Log Empty\r\n"));

        // Unencrypted session so the test needs no handshake.
        let session = Session::new(ReplayTransport::new(reports), 0x60, 0x60, false);
        let mut device = LibreDevice::from_session(session);

        let readings = device.read_all().unwrap();
        assert_eq!(readings.len(), 2);
        match &readings[0] {
            Reading::Glucose { value_mg_dl, source, device_id, .. } => {
                assert_eq!(*value_mg_dl, 101);
                assert_eq!(*source, GlucoseSource::SensorHistory);
                assert_eq!(*device_id, 12);
            }
            other => panic!("expected glucose, got {other:?}"),
        }
    }
}

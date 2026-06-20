//! Glucose data access for the in-app Health view.
//!
//! The real backend — reading the local Parquet store and syncing from the USB
//! reader — is **desktop-only**, gated to macOS/Windows/Linux. That keeps wasm
//! (web) and mobile (iOS/Android) builds free of Parquet/hidapi *and* free of the
//! Libre 2 device keys; on those targets [`load`] returns an `unsupported`
//! snapshot and the view shows a desktop-only note.

/// One glucose reading, flattened for display.
#[derive(Debug, Clone, PartialEq)]
pub struct GlucosePoint {
    /// Unix seconds (treating the device wall-clock as UTC) — for ordering and
    /// the sparkline x-axis.
    pub ts_unix: i64,
    /// Human label, e.g. `2026-06-19 08:32`.
    pub ts_label: String,
    /// Value in mg/dL (the device-internal unit).
    pub value: f64,
    /// Reading kind: `scan`, `sensor`, `blood`, or empty.
    pub kind: String,
    pub food: bool,
    pub exercise: bool,
}

/// A snapshot of stored glucose for the view.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GlucoseData {
    pub points: Vec<GlucosePoint>,
    pub unit: String,
    pub error: Option<String>,
    /// False on web/mobile (no local store or reader) → the view shows a note.
    pub supported: bool,
}

impl GlucoseData {
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    fn unsupported() -> Self {
        Self {
            points: Vec::new(),
            unit: "mg/dL".into(),
            error: None,
            supported: false,
        }
    }

    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    fn error(msg: String) -> Self {
        Self {
            points: Vec::new(),
            unit: "mg/dL".into(),
            error: Some(msg),
            supported: true,
        }
    }
}

/// Outcome of a reader sync.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncReport {
    pub serial: String,
    pub total: usize,
    pub added: usize,
}

// ---- Desktop backend ------------------------------------------------------

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
const GLUCOSE_STREAM: &str = "glucose.mg_dl";

/// Read all glucose observations from the local store.
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub fn load() -> GlucoseData {
    use looplace_store::{ParquetStore, Query, Store};

    let path = match crate::core::storage::data_dir() {
        Ok(dir) => dir.join("looplace.parquet"),
        Err(e) => return GlucoseData::error(format!("data dir unavailable: {e}")),
    };
    // `open` treats a missing file as an empty store, so first-run is not an error.
    let store = match ParquetStore::open(&path) {
        Ok(s) => s,
        Err(e) => return GlucoseData::error(format!("couldn't open store: {e}")),
    };
    let rows = match store.query(&Query::stream(GLUCOSE_STREAM)) {
        Ok(r) => r,
        Err(e) => return GlucoseData::error(format!("couldn't read glucose: {e}")),
    };
    let points = rows.iter().map(point_from_obs).collect();
    GlucoseData {
        points,
        unit: "mg/dL".into(),
        error: None,
        supported: true,
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn point_from_obs(o: &looplace_store::Observation) -> GlucosePoint {
    let is = |k: &str| o.tags.get(k).map(|v| v == "true").unwrap_or(false);
    GlucosePoint {
        ts_unix: o.timestamp.assume_utc().unix_timestamp(),
        ts_label: format_ts(o.timestamp),
        value: o.value,
        kind: o.tags.get("kind").cloned().unwrap_or_default(),
        food: is("food"),
        exercise: is("exercise"),
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn format_ts(t: time::PrimitiveDateTime) -> String {
    use time::macros::format_description;
    let fmt = format_description!("[year]-[month]-[day] [hour]:[minute]");
    t.format(&fmt).unwrap_or_else(|_| "—".into())
}

/// Pull every reading from a connected FreeStyle Libre 2 over USB and write them
/// into the local store. Blocking (USB handshake + multi-record reads) and
/// read-only against the device. **Private on purpose:** it must only ever run on
/// the [`device_thread`] — see that function for why.
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn sync_from_reader() -> std::result::Result<SyncReport, String> {
    use looplace_libre::LibreDevice;
    use looplace_store::convert::reading_to_observation;
    use looplace_store::{ParquetStore, Store};

    let dir = crate::core::storage::data_dir().map_err(|e| format!("data dir unavailable: {e}"))?;
    let mut store = ParquetStore::open(dir.join("looplace.parquet"))
        .map_err(|e| format!("couldn't open store: {e}"))?;

    let mut device = LibreDevice::open_libre2().map_err(|e| format!("reader not found: {e}"))?;
    device.connect().map_err(|e| format!("handshake failed: {e}"))?;
    let serial = device.serial_number().unwrap_or_else(|_| "unknown".into());
    let readings = device.read_all().map_err(|e| format!("read failed: {e}"))?;

    let observations: Vec<_> = readings
        .iter()
        .filter_map(|r| reading_to_observation(r, &serial))
        .collect();
    let total = observations.len();
    let added = store
        .upsert(&observations)
        .map_err(|e| format!("store write failed: {e}"))?;
    Ok(SyncReport {
        serial,
        total,
        added,
    })
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
type SyncReply = futures_channel::oneshot::Sender<std::result::Result<SyncReport, String>>;

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
enum DeviceCmd {
    Sync(SyncReply),
}

/// A single long-lived thread that owns **all** hidapi/IOKit interaction.
///
/// macOS pins the `IOHIDManager` to the `CFRunLoop` of the thread that created
/// it; touching it from another thread — or after that thread has exited — taps
/// a dangling run-loop source and traps (`__CFCheckCFInfoPACSignature`). The UI
/// spawns a fresh worker per click, so serializing every sync onto one stable
/// thread is what keeps the run loop valid across repeated syncs.
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn device_thread() -> &'static std::sync::Mutex<std::sync::mpsc::Sender<DeviceCmd>> {
    use std::sync::{mpsc, Mutex, OnceLock};
    static TX: OnceLock<Mutex<mpsc::Sender<DeviceCmd>>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<DeviceCmd>();
        std::thread::Builder::new()
            .name("looplace-device".into())
            .spawn(move || {
                while let Ok(cmd) = rx.recv() {
                    match cmd {
                        DeviceCmd::Sync(reply) => {
                            let _ = reply.send(sync_from_reader());
                        }
                    }
                }
            })
            .expect("spawn looplace-device thread");
        Mutex::new(tx)
    })
}

/// Enqueue a reader sync on the [`device_thread`]; `await` the returned receiver
/// on the UI task. Resolves to canceled if the device thread can't be reached.
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub fn request_sync() -> futures_channel::oneshot::Receiver<std::result::Result<SyncReport, String>>
{
    let (tx, rx) = futures_channel::oneshot::channel();
    if let Ok(sender) = device_thread().lock() {
        let _ = sender.send(DeviceCmd::Sync(tx));
    }
    rx
}

// ---- Non-desktop stub (web / mobile) --------------------------------------

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub fn load() -> GlucoseData {
    GlucoseData::unsupported()
}

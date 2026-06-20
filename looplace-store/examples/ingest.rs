// SPDX-License-Identifier: Apache-2.0
//! End-to-end ingest: pull the FreeStyle Libre 2 reader **and** migrate the
//! legacy cognition store into ONE unified Parquet file.
//!
//! ```text
//! cargo run -p looplace-store --features ingest --example ingest [out.parquet]
//! ```
//!
//! Reads the reader (USB) and the legacy `summaries.json` (read-only). Writes the
//! store, backup, and marker to the **output directory** — demo-safe: it does not
//! touch the real app data dir's marker/backup. Default output: `./looplace.parquet`.

#[cfg(feature = "ingest")]
fn main() {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use looplace_store::convert::reading_to_observation;
    use looplace_store::migrate::{
        run_upgrade, MigrationOutcome, MigrationPlan, LEGACY_FILE, MARKER_FILE,
    };
    use looplace_store::{ParquetStore, Query, Store};

    let out_path: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("looplace.parquet"));
    let out_dir = out_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let mut store = match ParquetStore::open(&out_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("✗ could not open store at {}: {e}", out_path.display());
            std::process::exit(1);
        }
    };

    // 1. Migrate legacy cognition. Source = $LOOPLACE_LEGACY_SUMMARIES (handy for
    //    restoring from a backup), else the real app data dir. Backup + marker go
    //    to the output dir, so the real app data dir is never modified.
    let legacy_summaries = std::env::var_os("LOOPLACE_LEGACY_SUMMARIES")
        .map(PathBuf::from)
        .or_else(|| {
            directories::ProjectDirs::from("com", "Looplace", "Looplace")
                .map(|d| d.data_dir().join(LEGACY_FILE))
        });
    if let Some(legacy_summaries) = legacy_summaries {
        let plan = MigrationPlan {
            legacy_summaries: legacy_summaries.clone(),
            backup_path: out_dir.join(format!("{LEGACY_FILE}.pre-store-backup-demo")),
            marker: out_dir.join(MARKER_FILE),
        };
        match run_upgrade(&plan, &mut store) {
            Ok(MigrationOutcome::Migrated(r)) => eprintln!(
                "✓ migrated {} cognition sessions ({} observations) from {}",
                r.sessions,
                r.observations_inserted,
                legacy_summaries.display()
            ),
            Ok(MigrationOutcome::AlreadyDone) => {
                eprintln!("• cognition already migrated (marker in {})", out_dir.display())
            }
            Ok(MigrationOutcome::NothingToMigrate) => eprintln!(
                "• no legacy summaries.json at {} — nothing to migrate",
                legacy_summaries.display()
            ),
            Err(e) => eprintln!("✗ cognition migration: {e}"),
        }
    }

    // 2. Ingest glucose from the reader (skip gracefully if it isn't connected).
    eprintln!("Opening FreeStyle Libre 2 reader …");
    match open_and_connect() {
        Ok(mut device) => {
            let serial = device.serial_number().unwrap_or_else(|_| "unknown".into());
            let tz = iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".into());
            match device.read_all() {
                Ok(readings) => {
                    let observations: Vec<_> = readings
                        .iter()
                        .filter_map(|r| reading_to_observation(r, &serial, &tz))
                        .collect();
                    match store.upsert(&observations) {
                        Ok(new) => eprintln!(
                            "✓ ingested {} glucose observations ({new} new) from reader {serial}",
                            observations.len()
                        ),
                        Err(e) => eprintln!("✗ store upsert: {e}"),
                    }
                }
                Err(e) => eprintln!("✗ read_all: {e}"),
            }
        }
        Err(e) => eprintln!("⚠ reader unavailable ({e}) — store still holds migrated cognition data"),
    }

    // 3. Summarize the unified store.
    let all = store.query(&Query::default()).unwrap_or_default();
    let mut per_stream: BTreeMap<String, usize> = BTreeMap::new();
    for o in &all {
        *per_stream.entry(o.stream.clone()).or_default() += 1;
    }
    eprintln!("\n=== unified store: {} ({} observations) ===", out_path.display(), all.len());
    for (stream, n) in &per_stream {
        eprintln!("  {stream:<28} {n}");
    }
    eprintln!(
        "\nExplore it:\n  duckdb -c \"SELECT stream, count(*) FROM '{}' GROUP BY 1 ORDER BY 1\"",
        out_path.display()
    );
}

#[cfg(feature = "ingest")]
fn open_and_connect() -> looplace_libre::Result<looplace_libre::LibreDevice<looplace_libre::transport::HidApiTransport>> {
    let mut device = looplace_libre::LibreDevice::open_libre2()?;
    device.connect()?;
    Ok(device)
}

#[cfg(not(feature = "ingest"))]
fn main() {
    eprintln!(
        "ingest needs the `ingest` feature (forwards reader transport + keys) and a connected reader:\n\
         \n    cargo run -p looplace-store --features ingest --example ingest [out.parquet]\n"
    );
    std::process::exit(2);
}

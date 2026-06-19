//! One-time upgrade path: import the legacy `summaries.json` into a [`Store`].
//!
//! The app calls [`run_upgrade`] on **every** startup; it migrates exactly once
//! (gated by a marker file), is safe to re-run, and never deletes the original —
//! it backs it up first. This is the flow that upgrades existing users when they
//! first launch the storage-crate version.

use std::fs;
use std::path::{Path, PathBuf};

use crate::convert::{summaries_from_json, summary_to_observations};
use crate::error::Result;
use crate::store::Store;

/// Legacy cognition store filename (web localStorage key `looplace_summaries`;
/// on desktop, this file in the app data dir).
pub const LEGACY_FILE: &str = "summaries.json";
/// Marker recording that the cognition migration has completed.
pub const MARKER_FILE: &str = ".cognition-migrated";

/// The concrete paths a migration operates on. Decoupled from any specific app
/// layout so it stays testable; use [`MigrationPlan::for_data_dir`] for the
/// standard Looplace layout.
#[derive(Debug, Clone)]
pub struct MigrationPlan {
    /// Legacy `summaries.json` (may be absent on a fresh install).
    pub legacy_summaries: PathBuf,
    /// Where the original is copied before import.
    pub backup_path: PathBuf,
    /// Idempotency marker; when present, [`run_upgrade`] is a no-op.
    pub marker: PathBuf,
}

impl MigrationPlan {
    /// Standard paths within an app data directory. `tag` (e.g. a timestamp)
    /// keeps the backup filename unique and legible.
    pub fn for_data_dir(data_dir: &Path, tag: &str) -> Self {
        Self {
            legacy_summaries: data_dir.join(LEGACY_FILE),
            backup_path: data_dir.join(format!("{LEGACY_FILE}.pre-store-backup-{tag}")),
            marker: data_dir.join(MARKER_FILE),
        }
    }
}

/// Details of an import.
#[derive(Debug, Clone, Default)]
pub struct MigrationReport {
    pub backup_path: Option<PathBuf>,
    /// Cognition sessions read from the legacy file.
    pub sessions: usize,
    /// New observation rows written.
    pub observations_inserted: usize,
}

/// What [`run_upgrade`] did on this startup.
#[derive(Debug, Clone)]
pub enum MigrationOutcome {
    /// Marker already present — migration ran on a previous launch.
    AlreadyDone,
    /// No legacy file (fresh install). Marker written so we don't re-check.
    NothingToMigrate,
    /// Legacy data backed up and imported.
    Migrated(MigrationReport),
}

/// Run the one-time legacy → store upgrade. Safe and idempotent to call on every
/// app startup.
///
/// Order is crash-safe: back up first, then import (idempotent upsert), then
/// write the marker. A crash before the marker simply re-runs harmlessly.
pub fn run_upgrade(plan: &MigrationPlan, store: &mut dyn Store) -> Result<MigrationOutcome> {
    if plan.marker.exists() {
        return Ok(MigrationOutcome::AlreadyDone);
    }

    if !plan.legacy_summaries.exists() {
        write_marker(&plan.marker, "no legacy summaries.json; nothing to migrate")?;
        return Ok(MigrationOutcome::NothingToMigrate);
    }

    // 1. Back up the original before touching anything; never modify/delete it.
    if let Some(parent) = plan.backup_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&plan.legacy_summaries, &plan.backup_path)?;

    // 2. Import (idempotent upsert).
    let mut report = import_summaries(&plan.legacy_summaries, store)?;
    report.backup_path = Some(plan.backup_path.clone());

    // 3. Mark done so future launches skip straight to AlreadyDone.
    write_marker(
        &plan.marker,
        &format!(
            "migrated {} sessions, {} observations; backup: {}",
            report.sessions,
            report.observations_inserted,
            plan.backup_path.display()
        ),
    )?;

    Ok(MigrationOutcome::Migrated(report))
}

/// Parse a legacy `summaries.json` and upsert its metrics into `store`. The
/// reusable primitive — no backup, no marker.
pub fn import_summaries(path: &Path, store: &mut dyn Store) -> Result<MigrationReport> {
    let raw = fs::read_to_string(path)?;
    let summaries = summaries_from_json(&raw)?;
    let observations: Vec<_> = summaries.iter().flat_map(summary_to_observations).collect();
    let observations_inserted = store.upsert(&observations)?;
    Ok(MigrationReport {
        backup_path: None,
        sessions: summaries.len(),
        observations_inserted,
    })
}

fn write_marker(path: &Path, note: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, note)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observation::Query;
    use crate::store::MemoryStore;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("looplace_migrate_{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    const SAMPLE: &str = r#"[
        {"id":"pvt-1","task":"pvt","created_at":"2026-06-19T08:00:00Z",
         "metrics":{"median_rt_ms":312.5,"lapses_ge_500ms":2}},
        {"id":"nback2-1","task":"nback2","created_at":"2026-06-19T09:00:00Z",
         "metrics":{"dprime":1.8,"accuracy_pct":92.0}}
    ]"#;

    #[test]
    fn upgrades_once_then_is_a_noop() {
        let dir = temp_dir("upgrade");
        fs::write(dir.join(LEGACY_FILE), SAMPLE).unwrap();
        let plan = MigrationPlan::for_data_dir(&dir, "20260619T120000Z");
        let mut store = MemoryStore::new();

        // First launch: migrates.
        match run_upgrade(&plan, &mut store).unwrap() {
            MigrationOutcome::Migrated(r) => {
                assert_eq!(r.sessions, 2);
                assert_eq!(r.observations_inserted, 4);
            }
            other => panic!("expected Migrated, got {other:?}"),
        }
        // Original untouched, backup + marker written, data queryable.
        assert!(plan.legacy_summaries.exists());
        assert!(plan.backup_path.exists());
        assert!(plan.marker.exists());
        assert_eq!(store.query(&Query::stream("nback2.dprime")).unwrap()[0].value, 1.8);

        // Second launch: marker present → no-op (no double import).
        assert!(matches!(
            run_upgrade(&plan, &mut store).unwrap(),
            MigrationOutcome::AlreadyDone
        ));
        assert_eq!(store.query(&Query::default()).unwrap().len(), 4);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn fresh_install_writes_marker_and_skips() {
        let dir = temp_dir("fresh");
        let plan = MigrationPlan::for_data_dir(&dir, "tag");
        let mut store = MemoryStore::new();

        assert!(matches!(
            run_upgrade(&plan, &mut store).unwrap(),
            MigrationOutcome::NothingToMigrate
        ));
        assert!(plan.marker.exists());
        assert!(store.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_summaries_is_idempotent() {
        let dir = temp_dir("import");
        let path = dir.join(LEGACY_FILE);
        fs::write(&path, SAMPLE).unwrap();
        let mut store = MemoryStore::new();

        assert_eq!(import_summaries(&path, &mut store).unwrap().observations_inserted, 4);
        assert_eq!(import_summaries(&path, &mut store).unwrap().observations_inserted, 0);

        let _ = fs::remove_dir_all(&dir);
    }
}

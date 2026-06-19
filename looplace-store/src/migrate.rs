//! Upgrade path for existing users: import the legacy `summaries.json` into a
//! [`Store`], backing up the original first and never deleting it.

use std::fs;
use std::path::{Path, PathBuf};

use crate::convert::{summaries_from_json, summary_to_observations};
use crate::error::Result;
use crate::store::Store;

/// What a migration did.
#[derive(Debug, Clone, Default)]
pub struct MigrationReport {
    /// `None` if there was nothing to migrate (no legacy file).
    pub backup_path: Option<PathBuf>,
    /// Cognition sessions read from the legacy file.
    pub sessions: usize,
    /// New observation rows written to the store.
    pub observations_inserted: usize,
}

impl MigrationReport {
    pub fn migrated(&self) -> bool {
        self.backup_path.is_some()
    }
}

/// Import `summaries.json` at `path` into `store`.
///
/// Safe and idempotent: the original is copied to
/// `summaries.json.backup-<tag>` first and left in place; re-running upserts the
/// same rows (no duplicates). `backup_tag` is supplied by the caller (e.g. a
/// timestamp) so this stays deterministic/testable.
pub fn migrate_summaries_json(
    path: &Path,
    store: &mut dyn Store,
    backup_tag: &str,
) -> Result<MigrationReport> {
    if !path.exists() {
        return Ok(MigrationReport::default());
    }

    let raw = fs::read_to_string(path)?;

    // Back up the original before anything else; never modify/delete it.
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("summaries.json");
    let backup_path = path.with_file_name(format!("{file_name}.backup-{backup_tag}"));
    fs::copy(path, &backup_path)?;

    let summaries = summaries_from_json(&raw)?;
    let observations: Vec<_> = summaries.iter().flat_map(summary_to_observations).collect();
    let observations_inserted = store.upsert(&observations)?;

    Ok(MigrationReport {
        backup_path: Some(backup_path),
        sessions: summaries.len(),
        observations_inserted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observation::Query;
    use crate::store::MemoryStore;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("looplace_store_test_{name}"));
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
    fn migrates_backs_up_and_is_idempotent() {
        let dir = temp_dir("migrate");
        let path = dir.join("summaries.json");
        fs::write(&path, SAMPLE).unwrap();

        let mut store = MemoryStore::new();
        let report = migrate_summaries_json(&path, &mut store, "20260619T120000Z").unwrap();

        assert!(report.migrated());
        assert_eq!(report.sessions, 2);
        assert_eq!(report.observations_inserted, 4); // 2 metrics each

        // Original is untouched; backup exists alongside it.
        assert!(path.exists());
        let backup = report.backup_path.unwrap();
        assert!(backup.exists());
        assert_eq!(fs::read_to_string(&backup).unwrap(), SAMPLE);

        // Data landed and is queryable.
        let dprime = store.query(&Query::stream("nback2.dprime")).unwrap();
        assert_eq!(dprime.len(), 1);
        assert_eq!(dprime[0].value, 1.8);

        // Re-running migrates no *new* rows (idempotent upsert).
        let again = migrate_summaries_json(&path, &mut store, "20260619T130000Z").unwrap();
        assert_eq!(again.observations_inserted, 0);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_legacy_file_is_a_noop() {
        let dir = temp_dir("noop");
        let mut store = MemoryStore::new();
        let report =
            migrate_summaries_json(&dir.join("missing.json"), &mut store, "tag").unwrap();
        assert!(!report.migrated());
        assert_eq!(report.observations_inserted, 0);
        let _ = fs::remove_dir_all(&dir);
    }
}

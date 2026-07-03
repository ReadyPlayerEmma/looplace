mod list;
pub use list::ResultsList;

mod detail;
pub use detail::ResultsDetailPanel;

mod charts;
pub use charts::ResultsSparklines;

mod export;
pub use export::ResultsExportPanel;

mod fonts;

mod utils;
pub(crate) use utils::*;

use crate::core::storage::{self, SummaryRecord};

/// Shared state for the results view aggregating stored summaries or load errors.
#[derive(Debug, Clone, Default)]
pub struct ResultsState {
    pub records: Vec<SummaryRecord>,
    pub error: Option<String>,
}

impl ResultsState {
    pub fn load() -> Self {
        // Desktop: the unified store is the source of truth (new runs are
        // written through to it on save). An empty store falls back to the
        // legacy JSON — e.g. the startup migration hasn't populated it yet —
        // and a store error falls back too, so results are never held hostage.
        #[cfg(all(
            feature = "store",
            any(target_os = "macos", target_os = "windows", target_os = "linux")
        ))]
        {
            match crate::core::local_store::load_summaries() {
                Ok(mut records) if !records.is_empty() => {
                    records.sort_by(|a, b| b.created_at.cmp(&a.created_at));
                    return Self {
                        records,
                        error: None,
                    };
                }
                Ok(_) => {}
                Err(err) => eprintln!("[store] results falling back to summaries.json: {err}"),
            }
        }

        match storage::load_summaries() {
            Ok(mut records) => {
                records.sort_by(|a, b| b.created_at.cmp(&a.created_at));
                Self {
                    records,
                    error: None,
                }
            }
            Err(err) => Self {
                records: Vec::new(),
                error: Some(format!("Couldn't load summaries: {err}")),
            },
        }
    }
}

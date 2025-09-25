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

//! Local persistence helpers for summaries and settings.

#[derive(Debug, Default, Clone)]
pub struct SummaryRecord {
    pub id: String,
    pub task: String,
}

impl SummaryRecord {
    pub fn new<T: Into<String>>(id: T, task: T) -> Self {
        Self {
            id: id.into(),
            task: task.into(),
        }
    }
}

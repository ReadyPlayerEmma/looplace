use dioxus::prelude::*;

use super::{NBackEngine, NBackMetrics};

#[component]
pub fn NBackView() -> Element {
    let engine = NBackEngine::new(30);
    let metrics = NBackMetrics::empty();

    rsx! {
        article { class: "task task-nback",
            h2 { "Session preview" }
            p { "Planned trials: {engine.planned_trials}" }
            p { "Hits: {metrics.hits}" }
            p { "False alarms: {metrics.false_alarms}" }
            p { class: "task__note",
                "Interactive stream forthcoming. Placeholder renders summary stats for now."
            }
        }
    }
}

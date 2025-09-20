use dioxus::prelude::*;

use super::{PvtEngine, PvtMetrics};

#[component]
pub fn PvtView() -> Element {
    let engine = PvtEngine::new(0);
    let metrics = PvtMetrics::empty();

    rsx! {
        article { class: "task task-pvt",
            h2 { "Session preview" }
            p { "Engine seed: {engine.seed}" }
            p { "Median RT (ms): {metrics.median_rt_ms:.0}" }
            p { "Lapses ≥500 ms: {metrics.lapses_ge_500ms}" }
            p { "False starts: {metrics.false_starts}" }
            p { class: "task__note",
                "Interactive task loop not implemented yet. This placeholder ensures the view compiles."
            }
        }
    }
}

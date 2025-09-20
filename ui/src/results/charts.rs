use dioxus::prelude::*;

#[component]
pub fn ResultsSparklines() -> Element {
    rsx! {
        section { class: "results-charts",
            h2 { "Trend snapshots" }
            p { "Charts will visualize reaction times, lapses, and d′ once metrics are wired." }
        }
    }
}

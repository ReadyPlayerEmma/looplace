use dioxus::prelude::*;

#[component]
pub fn ResultsDetailPanel() -> Element {
    rsx! {
        section { class: "results-detail",
            h2 { "Details" }
            p { "Select a run to review metrics, QC flags, and notes." }
        }
    }
}

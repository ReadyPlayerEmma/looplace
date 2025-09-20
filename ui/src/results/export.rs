use dioxus::prelude::*;

#[component]
pub fn ResultsExportPanel() -> Element {
    rsx! {
        section { class: "results-export",
            h2 { "Export" }
            p { "Share results as JSON, CSV, or PNG once export hooks are connected." }
        }
    }
}

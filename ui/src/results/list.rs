use dioxus::prelude::*;

#[component]
pub fn ResultsList() -> Element {
    rsx! {
        section { class: "results-list",
            h2 { "Recent runs" }
            p { "No runs recorded yet. Completed sessions will appear here." }
        }
    }
}

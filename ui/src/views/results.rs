use dioxus::prelude::*;

use crate::results::{ResultsDetailPanel, ResultsExportPanel, ResultsList, ResultsSparklines};

#[component]
pub fn Results() -> Element {
    rsx! {
        section { class: "page page-results",
            h1 { "Results" }
            p {
                "Review summaries from recent runs, inspect quality checks, and export data for deeper analysis."
            }

            div { class: "results__panels",
                ResultsList {}
                ResultsDetailPanel {}
            }

            ResultsSparklines {}
            ResultsExportPanel {}
        }
    }
}

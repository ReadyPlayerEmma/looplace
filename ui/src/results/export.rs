use dioxus::prelude::*;

use crate::core::storage::SummaryRecord;

#[component]
pub fn ResultsExportPanel(records: Vec<SummaryRecord>) -> Element {
    let total_runs = records.len();
    let pvt_runs = records.iter().filter(|r| r.task == "pvt").count();
    let nback_runs = records.iter().filter(|r| r.task == "nback2").count();

    rsx! {
        section { class: "results-card results-export",
            div { class: "results-card__header",
                h2 { "Export" }
            }

            if total_runs == 0 {
                p { class: "results-card__placeholder", "Exports unlock once runs are stored locally." }
            } else {
                p {
                    "Prepare tidy JSON or CSV summaries for deeper analysis. Exports will land in your downloads folder."
                }

                ul { class: "results-export__summary",
                    li { strong { "{total_runs}" } " total runs cached" }
                    li { strong { "{pvt_runs}" } " psychomotor vigilance" }
                    li { strong { "{nback_runs}" } " 2-back runs" }
                }

                div { class: "results-export__actions",
                    button {
                        r#type: "button",
                        class: "button button--primary",
                        disabled: true,
                        "Export JSON"
                    }
                    button {
                        r#type: "button",
                        class: "button",
                        disabled: true,
                        "Export CSV"
                    }
                }

                p { class: "results-card__meta", "Export hooks arrive in M3 once server functions are online." }
            }
        }
    }
}

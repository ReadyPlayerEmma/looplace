use dioxus::prelude::*;

use crate::results::{
    ResultsDetailPanel, ResultsExportPanel, ResultsList, ResultsSparklines, ResultsState,
};

#[component]
pub fn Results() -> Element {
    let results_state = use_signal(ResultsState::load);
    let mut selected_id = use_signal(|| Option::<String>::None);

    let snapshot = results_state();

    if selected_id().is_none() {
        if let Some(first) = snapshot.records.first() {
            selected_id.set(Some(first.id.clone()));
        }
    }

    let mut active_record = selected_id().and_then(|id| {
        snapshot
            .records
            .iter()
            .find(|record| record.id == id)
            .cloned()
    });

    if active_record.is_none() {
        if let Some(first) = snapshot.records.first() {
            let fallback_id = first.id.clone();
            selected_id.set(Some(fallback_id.clone()));
            active_record = Some(first.clone());
        }
    }

    let runs_count = snapshot.records.len();
    let refresh = {
        let mut results_signal = results_state;
        move |_| {
            results_signal.set(ResultsState::load());
        }
    };

    rsx! {
        section { class: "page page-results",
            div { class: "results__header",
                h1 { "Results" }
                button {
                    r#type: "button",
                    class: "button button--ghost",
                    onclick: refresh,
                    "Refresh"
                }
            }
            p { class: "results__intro",
                "Review summaries from recent runs, inspect quality checks, and export data for deeper analysis."
            }

            if let Some(err) = snapshot.error.clone() {
                div { class: "results__alert results__alert--error", "⚠️ {err}" }
            }
            if runs_count == 0 && snapshot.error.is_none() {
                div { class: "results__alert", "No runs recorded yet. Completed sessions will appear after you finish a task." }
            }

            div { class: "results__panels",
                ResultsList {
                    results: results_state,
                    selected_id: selected_id,
                }
                ResultsDetailPanel {
                    record: active_record,
                }
            }

            ResultsSparklines { records: snapshot.records.clone() }
            ResultsExportPanel { records: snapshot.records }
        }
    }
}

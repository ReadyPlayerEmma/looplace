use dioxus::prelude::*;

use crate::results::{
    ResultsDetailPanel, ResultsExportPanel, ResultsList, ResultsSparklines, ResultsState,
};

#[component]
pub fn Results() -> Element {
    // Subscribe to global language code (if provided) so this view re-renders
    // when the user switches locale while viewing Results.
    let _lang_code: Option<Signal<String>> = try_use_context::<Signal<String>>();
    let _lang_marker = _lang_code.as_ref().map(|s| s()).unwrap_or_default();

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
        // Hidden marker node ensures reactive dependency on language signal.
        div { style: "display:none", "{_lang_marker}" }
        section { class: "page page-results",
            div { class: "results__header",
                h1 { {crate::t!("results-title")} }
                button {
                    r#type: "button",
                    class: "button button--ghost",
                    onclick: refresh,
                    {crate::t!("results-refresh")}
                }
            }
            p { class: "results__intro",
                {crate::t!("results-page-intro")}
            }

            if let Some(err) = snapshot.error.clone() {
                div { class: "results__alert results__alert--error", {crate::t!("results-error-prefix")} " {err}" }
            }
            if runs_count == 0 && snapshot.error.is_none() {
                div { class: "results__alert", {crate::t!("results-empty")} }
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

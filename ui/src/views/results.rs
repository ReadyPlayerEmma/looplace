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
    // Initialize the selection to the first run *once*, via a non-subscribing
    // peek in the initializer — never write to the signal during render.
    let selected_id = use_signal(|| {
        results_state
            .peek()
            .records
            .first()
            .map(|record| record.id.clone())
    });

    let snapshot = results_state();

    // Resolve the active record by reading only: honor an explicit selection if it
    // still matches a run, else fall back to the first. No signal writes in the
    // component body (that previously risked an infinite re-render — see the
    // dioxus_signals "read and write in reactive scope" warning).
    let active_record = selected_id()
        .and_then(|id| snapshot.records.iter().find(|record| record.id == id).cloned())
        .or_else(|| snapshot.records.first().cloned());

    let runs_count = snapshot.records.len();

    // Reload runs and re-point the selection — a write in an event handler, which
    // is the safe place for it.
    let refresh = {
        let (mut state, mut selection) = (results_state, selected_id);
        move |_| {
            let reloaded = ResultsState::load();
            selection.set(reloaded.records.first().map(|record| record.id.clone()));
            state.set(reloaded);
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

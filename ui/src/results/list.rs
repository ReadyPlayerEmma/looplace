use crate::{
    core::{
        format,
        storage::{delete_summary, SummaryRecord},
    },
    results::{
        format_device, format_timestamp, parse_nback_metrics, parse_pvt_metrics, qc_summary,
        task_label, ResultsState,
    },
};
use dioxus::prelude::*;

#[cfg(target_arch = "wasm32")]
fn confirm_delete_prompt() -> bool {
    web_sys::window()
        .and_then(|w| {
            w.confirm_with_message("Delete this run permanently? This cannot be undone.")
                .ok()
        })
        .unwrap_or(false)
}

#[cfg(not(target_arch = "wasm32"))]
fn confirm_delete_prompt() -> bool {
    // Desktop (native) path: simple inline confirmation via stderr + always delete.
    // TODO: Replace with a custom in-app modal if stronger confirmation UX is desired.
    // For now we proceed without an interactive block dialog.
    true
}

#[component]
pub fn ResultsList(results: Signal<ResultsState>, selected_id: Signal<Option<String>>) -> Element {
    let state = results();
    let active_id = selected_id();

    let entries: Vec<ListEntry> = state
        .records
        .iter()
        .map(|record| {
            let id = record.id.clone();
            let is_active = active_id
                .as_ref()
                .map(|selected| selected == &id)
                .unwrap_or(false);

            let timestamp = format_timestamp(record);
            let metrics = metric_snippets(record);
            let qc = qc_summary(record);
            let label = task_label(&record.task).to_string();
            let device = format_device(&record.client.platform, &record.client.tz);

            ListEntry {
                id,
                is_active,
                timestamp,
                metrics,
                qc,
                task_label: label,
                device,
            }
        })
        .collect();

    rsx! {
        section { class: "results-card results-list",
            div { class: "results-card__header",
                h2 { "Recent runs" }
                if !state.records.is_empty() {
                    span { class: "results-card__meta", "{state.records.len()} recorded" }
                }
            }

            if state.records.is_empty() {
                p { class: "results-card__placeholder",
                    "Completed sessions will appear here once you wrap up a task run."
                }
            } else {
                ul { class: "results-list__items",
                    for entry in entries.into_iter() {
                        {render_list_entry(entry, selected_id, results)}
                    }
                }
            }
        }
    }
}

#[derive(Clone)]
struct ListEntry {
    id: String,
    is_active: bool,
    timestamp: String,
    metrics: Vec<(String, String)>,
    qc: String,
    task_label: String,
    device: Option<String>,
}

fn render_list_entry(
    entry: ListEntry,
    mut selected_id: Signal<Option<String>>,
    mut results: Signal<ResultsState>,
) -> Element {
    let ListEntry {
        id,
        is_active,
        timestamp,
        metrics,
        qc,
        task_label,
        device,
    } = entry;

    let button_id = id.clone();

    let delete_id = id.clone();
    rsx! {
        li { class: format!(
                "results-list__item {}",
                if is_active { "results-list__item--active" } else { "" }
            ),
            div { class: "results-list__row", style: "position:relative;",
                button {
                    r#type: "button",
                    class: "results-list__button",
                    onclick: move |_| selected_id.set(Some(button_id.clone())),

                    span { class: "results-list__heading",
                        span { class: "results-list__task", "{task_label}" }
                        span { class: "results-list__timestamp", "{timestamp}" }
                    }

                    if let Some(device_label) = device.as_ref() {
                        span { class: "results-list__device", "{device_label}" }
                    }

                    div { class: "results-list__metrics",
                        for (label, value) in metrics.iter() {
                            span { class: "results-list__metric",
                                span { class: "results-list__metric-label", "{label}" }
                                span { class: "results-list__metric-value", "{value}" }
                            }
                        }
                    }

                    span { class: "results-list__qc", "{qc}" }
                }
                button {
                    r#type: "button",
                    class: "results-list__delete",
                    style: "position:absolute;bottom:0.5rem;right:0.5rem;padding:0.35rem 0.4rem;margin:0;border:none;background:transparent;cursor:pointer;",
                    aria_label: "Delete run",
                    onclick: move |_| {
                        if !confirm_delete_prompt() {
                            return;
                        }
                        if delete_summary(&delete_id).unwrap_or(false) {
                            results.set(ResultsState::load());
                            if selected_id().as_ref().map(|x| x == &delete_id).unwrap_or(false) {
                                let state_now = results();
                                if let Some(first) = state_now.records.first() {
                                    selected_id.set(Some(first.id.clone()));
                                } else {
                                    selected_id.set(None);
                                }
                            }
                        }
                    },
                }
            }
        }
    }
}

fn metric_snippets(record: &SummaryRecord) -> Vec<(String, String)> {
    match record.task.as_str() {
        "pvt" => parse_pvt_metrics(record)
            .map(|metrics| {
                vec![
                    ("Median RT".into(), format::format_ms(metrics.median_rt_ms)),
                    ("Lapses".into(), metrics.lapses_ge_500ms.to_string()),
                    ("False starts".into(), metrics.false_starts.to_string()),
                ]
            })
            .unwrap_or_else(|| vec![("Metrics".into(), "Unavailable".into())]),
        "nback2" => parse_nback_metrics(record)
            .map(|metrics| {
                vec![
                    ("Accuracy".into(), format::format_percent(metrics.accuracy)),
                    ("d′".into(), format::format_number(metrics.d_prime, 2)),
                    ("Responses".into(), metrics.response_count.to_string()),
                ]
            })
            .unwrap_or_else(|| vec![("Metrics".into(), "Unavailable".into())]),
        _ => vec![("Task".into(), "Unknown".into())],
    }
}

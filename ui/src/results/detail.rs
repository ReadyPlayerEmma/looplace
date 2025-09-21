use dioxus::prelude::*;

use crate::{
    core::{format, storage::SummaryRecord},
    results::{
        format_device, format_timestamp, parse_nback_metrics, parse_pvt_metrics, qc_summary,
        task_label,
    },
};

#[component]
pub fn ResultsDetailPanel(record: Option<SummaryRecord>) -> Element {
    rsx! {
        section { class: "results-card results-detail",
            div { class: "results-card__header",
                h2 { "Details" }
            }

            match record {
                Some(record) => render_record(&record),
                None => rsx! {
                    p { class: "results-card__placeholder",
                        "Select a run to review metrics, quality checks, and device context."
                    }
                },
            }
        }
    }
}

fn render_record(record: &SummaryRecord) -> Element {
    let timestamp = format_timestamp(record);
    let qc = qc_summary(record);
    let device = format_device(&record.client.platform, &record.client.tz);
    let qc_trials_label = if record.qc.min_trials_met {
        "Yes"
    } else {
        "No"
    };

    let content = match record.task.as_str() {
        "pvt" => render_pvt(record),
        "nback2" => render_nback(record),
        _ => rsx! {
            p { class: "results-card__placeholder", "Metrics for this session aren't available yet." }
        },
    };

    rsx! {
        div { class: "results-detail__summary",
            h3 { "{task_label(&record.task)}" }
            span { class: "results-detail__timestamp", "{timestamp}" }
            if let Some(device_label) = device {
                span { class: "results-detail__device", "{device_label}" }
            }
        }

        {content}

        div { class: "results-detail__qc",
            h4 { "Quality checks" }
            ul {
                li { "{qc}" }
                li { "Focus lost events: {record.qc.focus_lost_events}" }
                li { "Window blur events: {record.qc.visibility_blur_events}" }
                li { "Minimum trials met: {qc_trials_label}" }
                li { "Captured platform: {record.client.platform}" }
            }
        }
    }
}

fn render_pvt(record: &SummaryRecord) -> Element {
    match parse_pvt_metrics(record) {
        Some(metrics) => {
            let min_trials_label = if metrics.meets_min_trial_requirement {
                "Yes"
            } else {
                "No"
            };
            rsx! {
                ul { class: "results-detail__grid",
                    li { span { class: "results-detail__metric-label", "Median RT" } span { class: "results-detail__metric-value", "{format::format_ms(metrics.median_rt_ms)}" } }
                    li { span { class: "results-detail__metric-label", "Mean RT" } span { class: "results-detail__metric-value", "{format::format_ms(metrics.mean_rt_ms)}" } }
                    li { span { class: "results-detail__metric-label", "SD" } span { class: "results-detail__metric-value", "{format::format_ms(metrics.sd_rt_ms)}" } }
                    li { span { class: "results-detail__metric-label", "P10" } span { class: "results-detail__metric-value", "{format::format_ms(metrics.p10_rt_ms)}" } }
                    li { span { class: "results-detail__metric-label", "P90" } span { class: "results-detail__metric-value", "{format::format_ms(metrics.p90_rt_ms)}" } }
                    li { span { class: "results-detail__metric-label", "Lapses ≥500 ms" } span { class: "results-detail__metric-value", "{metrics.lapses_ge_500ms}" } }
                    li { span { class: "results-detail__metric-label", "Minor lapses 355–499 ms" } span { class: "results-detail__metric-value", "{metrics.minor_lapses_355_499ms}" } }
                    li { span { class: "results-detail__metric-label", "False starts" } span { class: "results-detail__metric-value", "{metrics.false_starts}" } }
                    li { span { class: "results-detail__metric-label", "Slope" } span { class: "results-detail__metric-value", "{format::format_slope(metrics.time_on_task_slope_ms_per_min)}" } }
                    li {
                        span { class: "results-detail__metric-label", "Minimum trials met" }
                        span { class: "results-detail__metric-value", "{min_trials_label}" }
                    }
                }
            }
        }
        None => rsx! {
            p { class: "results-card__placeholder", "Unable to decode PVT metrics for this run." }
        },
    }
}

fn render_nback(record: &SummaryRecord) -> Element {
    match parse_nback_metrics(record) {
        Some(metrics) => {
            rsx! {
                ul { class: "results-detail__grid",
                    li { span { class: "results-detail__metric-label", "Accuracy" } span { class: "results-detail__metric-value", "{format::format_percent(metrics.accuracy)}" } }
                    li { span { class: "results-detail__metric-label", "d′" } span { class: "results-detail__metric-value", "{format::format_number(metrics.d_prime, 2)}" } }
                    li { span { class: "results-detail__metric-label", "Criterion" } span { class: "results-detail__metric-value", "{format::format_number(metrics.criterion, 2)}" } }
                    li { span { class: "results-detail__metric-label", "Hits" } span { class: "results-detail__metric-value", "{metrics.hits}" } }
                    li { span { class: "results-detail__metric-label", "Misses" } span { class: "results-detail__metric-value", "{metrics.misses}" } }
                    li { span { class: "results-detail__metric-label", "False alarms" } span { class: "results-detail__metric-value", "{metrics.false_alarms}" } }
                    li { span { class: "results-detail__metric-label", "Correct rejections" } span { class: "results-detail__metric-value", "{metrics.correct_rejections}" } }
                    li { span { class: "results-detail__metric-label", "Median hit RT" } span { class: "results-detail__metric-value", "{format::format_ms(metrics.median_hit_rt_ms)}" } }
                    li { span { class: "results-detail__metric-label", "Mean hit RT" } span { class: "results-detail__metric-value", "{format::format_ms(metrics.mean_hit_rt_ms)}" } }
                    li { span { class: "results-detail__metric-label", "Responses" } span { class: "results-detail__metric-value", "{metrics.response_count}" } }
                }
            }
        }
        None => rsx! {
            p { class: "results-card__placeholder", "Unable to decode 2-back metrics for this run." }
        },
    }
}

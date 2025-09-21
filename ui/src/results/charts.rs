use dioxus::prelude::*;

use crate::{
    core::{format, storage::SummaryRecord},
    results::{format_timestamp, parse_nback_metrics, parse_pvt_metrics},
};

#[component]
pub fn ResultsSparklines(records: Vec<SummaryRecord>) -> Element {
    let total_runs = records.len();
    let latest_stamp = records.first().map(format_timestamp);
    let latest_meta = latest_stamp.unwrap_or_default();

    let mut pvt_medians = Vec::new();
    let mut nback_accuracy = Vec::new();
    let mut nback_dprime = Vec::new();

    for record in &records {
        match record.task.as_str() {
            "pvt" => {
                if let Some(metrics) = parse_pvt_metrics(record) {
                    if metrics.median_rt_ms.is_finite() {
                        pvt_medians.push(metrics.median_rt_ms);
                    }
                }
            }
            "nback2" => {
                if let Some(metrics) = parse_nback_metrics(record) {
                    if metrics.accuracy.is_finite() {
                        nback_accuracy.push(metrics.accuracy);
                    }
                    if metrics.d_prime.is_finite() {
                        nback_dprime.push(metrics.d_prime);
                    }
                }
            }
            _ => {}
        }
    }

    let avg_pvt_median = average(&pvt_medians);
    let avg_nback_accuracy = average(&nback_accuracy);
    let avg_nback_dprime = average(&nback_dprime);

    let pvt_runs = pvt_medians.len();
    let nback_runs = nback_accuracy.len();

    let pvt_meta_text = if pvt_runs > 0 {
        "Average of recent PVT runs"
    } else {
        "Run a PVT to populate"
    };

    let nback_accuracy_meta = if nback_runs > 0 {
        "Mean accuracy across runs"
    } else {
        "Complete a 2-back session"
    };

    let dprime_meta = if nback_runs > 0 {
        "Signal detection over time"
    } else {
        "Data pending"
    };

    rsx! {
        section { class: "results-card results-charts",
            div { class: "results-card__header",
                h2 { "Highlights" }
                if total_runs > 0 {
                    span { class: "results-card__meta", "Latest run {latest_meta}" }
                }
            }

            if total_runs == 0 {
                p { class: "results-card__placeholder", "Once you complete tasks, we'll surface quick stats here." }
            } else {
                div { class: "results-highlights",
                    div { class: "results-highlight",
                        span { class: "results-highlight__label", "Total runs" }
                        strong { class: "results-highlight__value", "{total_runs}" }
                        span { class: "results-highlight__meta", "{pvt_runs} PVT · {nback_runs} 2-back" }
                    }
                    div { class: "results-highlight",
                        span { class: "results-highlight__label", "Median PVT" }
                        strong { class: "results-highlight__value", "{format::format_ms(avg_pvt_median)}" }
                        span { class: "results-highlight__meta", "{pvt_meta_text}" }
                    }
                    div { class: "results-highlight",
                        span { class: "results-highlight__label", "2-back accuracy" }
                        strong { class: "results-highlight__value", "{format::format_percent(avg_nback_accuracy)}" }
                        span { class: "results-highlight__meta", "{nback_accuracy_meta}" }
                    }
                    div { class: "results-highlight",
                        span { class: "results-highlight__label", "Average d′" }
                        strong { class: "results-highlight__value", "{format::format_number(avg_nback_dprime, 2)}" }
                        span { class: "results-highlight__meta", "{dprime_meta}" }
                    }
                }
            }
        }
    }
}

fn average(values: &[f64]) -> f64 {
    if values.is_empty() {
        f64::NAN
    } else {
        values.iter().copied().sum::<f64>() / values.len() as f64
    }
}

use std::fmt::Write;

use dioxus::prelude::*;

use crate::{
    core::{format, storage::SummaryRecord},
    results::{
        format_date_badge, format_time_badge, format_timestamp, parse_nback_metrics,
        parse_pvt_metrics, parse_timestamp, record_is_clean,
    },
};

#[component]
pub fn ResultsSparklines(records: Vec<SummaryRecord>) -> Element {
    let total_runs = records.len();
    let latest_stamp = records.first().map(format_timestamp);
    let latest_meta = latest_stamp.unwrap_or_default();

    let mut pvt_points = Vec::new();
    let mut nback_accuracy = Vec::new();
    let mut nback_dprime = Vec::new();

    let mut trend_points = Vec::new();
    let mut bar_points = Vec::new();

    let mut clean_total = 0usize;
    let mut clean_pvt = 0usize;
    let mut clean_nback = 0usize;

    for record in records.iter().rev() {
        if !record_is_clean(record) {
            continue;
        }

        clean_total += 1;

        if let Some(ts) = parse_timestamp(record) {
            match record.task.as_str() {
                "pvt" => {
                    if let Some(metrics) = parse_pvt_metrics(record) {
                        if metrics.median_rt_ms.is_finite() {
                            pvt_points.push(metrics.median_rt_ms);
                            trend_points.push(SparkPoint {
                                value: metrics.median_rt_ms,
                                badge: format_date_badge(ts),
                            });
                        }

                        bar_points.push(BarPoint {
                            badge: format_time_badge(ts),
                            lapses: metrics.lapses_ge_500ms,
                            false_starts: metrics.false_starts,
                        });
                        clean_pvt += 1;
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
                        clean_nback += 1;
                    }
                }
                _ => {}
            }
        }
    }

    let avg_pvt_median = average(&pvt_points);
    let avg_nback_accuracy = average(&nback_accuracy);
    let avg_nback_dprime = average(&nback_dprime);

    let pvt_runs = pvt_points.len();
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

    let sparkline = build_sparkline(&trend_points);
    let bar_chart = build_dual_bars(&bar_points);

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
                        span { class: "results-highlight__meta", "{clean_total} clean" }
                    }
                    div { class: "results-highlight",
                        span { class: "results-highlight__label", "Median PVT" }
                        strong { class: "results-highlight__value", "{format::format_ms(avg_pvt_median)}" }
                        span { class: "results-highlight__meta", "{pvt_meta_text} ({clean_pvt} clean)" }
                    }
                    div { class: "results-highlight",
                        span { class: "results-highlight__label", "2-back accuracy" }
                        strong { class: "results-highlight__value", "{format::format_percent(avg_nback_accuracy)}" }
                        span { class: "results-highlight__meta", "{nback_accuracy_meta} ({clean_nback} clean)" }
                    }
                    div { class: "results-highlight",
                        span { class: "results-highlight__label", "Average d′" }
                        strong { class: "results-highlight__value", "{format::format_number(avg_nback_dprime, 2)}" }
                        span { class: "results-highlight__meta", "{dprime_meta}" }
                    }
                }

                div { class: "results-charts__grid",
                    div { class: "results-chart results-chart--sparkline",
                        div { class: "results-chart__header",
                            span { class: "results-chart__title", "Median reaction time" }
                            span { class: "results-chart__meta", "PVT trend" }
                        }
                        if let Some(chart) = sparkline {
                            svg {
                                class: "results-chart__svg",
                                view_box: "0 0 360 120",
                                preserve_aspect_ratio: "none",
                                defs {
                                    linearGradient { id: "sparkline-fill", x1: "0", x2: "0", y1: "0", y2: "1",
                                        stop { offset: "0%", stop_color: "rgba(240,90,126,0.45)" }
                                        stop { offset: "100%", stop_color: "rgba(240,90,126,0.0)" }
                                    }
                                }
                                path { d: "{chart.fill_path}", fill: "url(#sparkline-fill)" }
                                path { d: "{chart.path}", fill: "none", stroke: "#f05a7e", stroke_width: "3", stroke_linecap: "round" }
                                if let Some((start, end)) = chart.labels {
                                    text { x: "0", y: "118", class: "results-chart__axis", "{start}" }
                                    text { x: "360", y: "118", class: "results-chart__axis", text_anchor: "end", "{end}" }
                                }
                            }
                            div { class: "results-chart__footer",
                                span { "Min {format::format_ms(chart.min)}" }
                                span { "Max {format::format_ms(chart.max)}" }
                            }
                        } else {
                            p { class: "results-card__placeholder", "Complete more PVT runs to unlock the trend." }
                        }
                    }

                    div { class: "results-chart results-chart--bars",
                        div { class: "results-chart__header",
                            span { class: "results-chart__title", "Lapses & false starts" }
                            span { class: "results-chart__meta", "Last 8 PVT sessions" }
                        }
                        if let Some(chart) = bar_chart {
                            svg {
                                class: "results-chart__svg",
                                view_box: "0 0 {chart.width} 140",
                                preserve_aspect_ratio: "none",
                                for bar in chart.bars.iter() {
                                    rect {
                                        x: "{bar.x}",
                                        y: "{bar.y}",
                                        width: "{bar.width}",
                                        height: "{bar.height}",
                                        rx: "4",
                                        fill: "{bar.color}",
                                    }
                                }
                                for label in chart.labels.iter() {
                                    text {
                                        x: "{label.x}",
                                        y: "135",
                                        class: "results-chart__axis",
                                        text_anchor: "middle",
                                        "{label.text}"
                                    }
                                }
                            }
                            div { class: "results-chart__legend",
                                span { class: "results-chart__legend-item",
                                    span { class: "results-chart__legend-swatch results-chart__legend-swatch--lapses" }
                                    span { "Lapses ≥500 ms" }
                                }
                                span { class: "results-chart__legend-item",
                                    span { class: "results-chart__legend-swatch results-chart__legend-swatch--false" }
                                    span { "False starts" }
                                }
                            }
                        } else {
                            p { class: "results-card__placeholder", "Run a few PVT sessions to populate lapse totals." }
                        }
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

struct SparkPoint {
    value: f64,
    badge: String,
}

struct SparklineChart {
    path: String,
    fill_path: String,
    min: f64,
    max: f64,
    labels: Option<(String, String)>,
}

fn build_sparkline(points: &[SparkPoint]) -> Option<SparklineChart> {
    if points.len() < 2 {
        return None;
    }

    let min = points
        .iter()
        .filter_map(|p| {
            if p.value.is_finite() {
                Some(p.value)
            } else {
                None
            }
        })
        .fold(f64::INFINITY, f64::min);
    let max = points
        .iter()
        .filter_map(|p| {
            if p.value.is_finite() {
                Some(p.value)
            } else {
                None
            }
        })
        .fold(f64::NEG_INFINITY, f64::max);

    if !min.is_finite() || !max.is_finite() {
        return None;
    }

    let span = (max - min).max(1.0);
    let width = 360.0;
    let height = 110.0;
    let step = width / (points.len() - 1) as f64;

    let mut path = String::new();
    let mut fill_path = String::new();

    let mut last_x = 0.0;

    for (index, point) in points.iter().enumerate() {
        let x = step * index as f64;
        let norm = ((point.value - min) / span).clamp(0.0, 1.0);
        let y = height - (norm * height);

        if index == 0 {
            let _ = write!(path, "M{:.2} {:.2}", x, y);
            let _ = write!(fill_path, "M{:.2} {:.2}", x, height);
            let _ = write!(fill_path, " L{:.2} {:.2}", x, y);
        } else {
            let _ = write!(path, " L{:.2} {:.2}", x, y);
            let _ = write!(fill_path, " L{:.2} {:.2}", x, y);
        }

        last_x = x;
    }

    let _ = write!(fill_path, " L{:.2} {:.2} Z", last_x, height);

    let labels = points
        .first()
        .zip(points.last())
        .map(|(start, end)| (start.badge.clone(), end.badge.clone()));

    Some(SparklineChart {
        path,
        fill_path,
        min,
        max,
        labels,
    })
}

#[derive(Clone)]
struct BarPoint {
    badge: String,
    lapses: u32,
    false_starts: u32,
}

struct BarRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    color: &'static str,
}

struct BarLabel {
    x: f64,
    text: String,
}

struct DualBarChart {
    width: f64,
    bars: Vec<BarRect>,
    labels: Vec<BarLabel>,
}

fn build_dual_bars(points: &[BarPoint]) -> Option<DualBarChart> {
    if points.is_empty() {
        return None;
    }

    let window_points: Vec<_> = points.iter().rev().take(8).cloned().collect();
    let mut points = window_points;
    points.reverse();

    let max_value = points
        .iter()
        .map(|p| p.lapses.max(p.false_starts))
        .max()
        .unwrap_or(0)
        .max(1) as f64;

    let height = 110.0;
    let margin = 16.0;
    let group_gap = 18.0;
    let bar_width = 12.0;
    let group_width = (bar_width * 2.0) + 6.0;
    let total_width = margin * 2.0 + (points.len() as f64 * (group_width + group_gap)) - group_gap;

    let mut bars = Vec::new();
    let mut labels = Vec::new();

    for (index, point) in points.iter().enumerate() {
        let group_x = margin + index as f64 * (group_width + group_gap);

        let lapses_height = if point.lapses == 0 {
            0.0
        } else {
            (point.lapses as f64 / max_value) * height
        };

        let false_height = if point.false_starts == 0 {
            0.0
        } else {
            (point.false_starts as f64 / max_value) * height
        };

        let lapses_y = height - lapses_height;
        let false_y = height - false_height;

        bars.push(BarRect {
            x: group_x,
            y: lapses_y,
            width: bar_width,
            height: lapses_height,
            color: "rgba(240,90,126,0.9)",
        });

        bars.push(BarRect {
            x: group_x + bar_width + 6.0,
            y: false_y,
            width: bar_width,
            height: false_height,
            color: "rgba(240,90,126,0.32)",
        });

        labels.push(BarLabel {
            x: group_x + group_width / 2.0,
            text: point.badge.clone(),
        });
    }

    Some(DualBarChart {
        width: total_width.max(320.0),
        bars,
        labels,
    })
}

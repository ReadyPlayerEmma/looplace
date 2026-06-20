//! The in-app Glucose (health) view: latest reading, a trend chart (with day +
//! value gridlines and a personal normal-range band), and a recent-readings list.
//!
//! On desktop, "Sync from reader" pulls the FreeStyle Libre 2 over USB on a shared
//! device thread (so the UI never freezes) and writes the local store. On
//! web/mobile the view shows a desktop-only note instead of a sync button.

use dioxus::prelude::*;

use crate::core::glucose::{self, GlucoseData, GlucosePoint, GlucoseSettings};

#[derive(Clone, PartialEq)]
enum SyncStatus {
    Idle,
    Running,
    Done {
        serial: String,
        added: usize,
        total: usize,
    },
    Error(String),
}

#[component]
pub fn Glucose() -> Element {
    // Subscribe to the global language code (if provided) so the view re-renders
    // when the locale changes — mirrors Results.
    let _lang_code: Option<Signal<String>> = try_use_context::<Signal<String>>();
    let _lang_marker = _lang_code.as_ref().map(|s| s()).unwrap_or_default();

    let data = use_signal(glucose::load);
    let sync_status = use_signal(|| SyncStatus::Idle);
    let settings = use_signal(glucose::load_settings);
    let hovered = use_signal(|| None::<usize>);

    let snapshot = data();
    let count = snapshot.points.len();
    let latest = snapshot.points.last().cloned();

    rsx! {
        div { style: "display:none", "{_lang_marker}" }
        section { class: "page page-glucose",
            div { class: "results__header",
                h1 { {crate::t!("glucose-title")} }
                {sync_action(data, sync_status)}
            }
            p { class: "results__intro", {crate::t!("glucose-intro")} }

            {status_banner(&sync_status())}

            if let Some(err) = snapshot.error.clone() {
                div { class: "results__alert results__alert--error",
                    {crate::t!("results-error-prefix")} " {err}"
                }
            }

            if !snapshot.supported {
                div { class: "results__alert", {crate::t!("glucose-desktop-only")} }
            } else if count == 0 && snapshot.error.is_none() {
                div { class: "results__alert", {crate::t!("glucose-empty")} }
            }

            if let Some(p) = latest {
                {latest_card(&p, &snapshot.unit)}
            }
            if snapshot.supported && count > 0 {
                {normal_range_editor(settings, snapshot.unit.clone())}
            }
            if count >= 2 {
                {glucose_chart(&snapshot.points, &snapshot.unit, settings(), hovered)}
            }
            if count > 0 {
                {recent_list(&snapshot.points, &snapshot.unit)}
            }
        }
    }
}

/// Transient sync feedback. English-only by design (these are ephemeral toasts
/// carrying live device values; the persistent page chrome is localized).
fn status_banner(status: &SyncStatus) -> Element {
    match status {
        SyncStatus::Idle => rsx! {},
        SyncStatus::Running => rsx! {
            div {
                class: "results__alert results__alert--info",
                "Syncing from reader… keep it connected."
            }
        },
        SyncStatus::Done { serial, added, total } => rsx! {
            div {
                class: "results__alert results__alert--success",
                "Synced {added} new of {total} readings from {serial}."
            }
        },
        SyncStatus::Error(msg) => rsx! {
            div { class: "results__alert results__alert--error", "Sync failed: {msg}" }
        },
    }
}

fn badge(text: &str) -> Element {
    rsx! {
        span {
            style: "font-size:0.8rem;padding:0.1rem 0.55rem;border-radius:999px;background:#eef2f7;color:#475467;",
            "{text}"
        }
    }
}

fn latest_card(p: &GlucosePoint, unit: &str) -> Element {
    rsx! {
        div {
            style: "display:flex;align-items:baseline;gap:0.75rem;flex-wrap:wrap;margin:1rem 0 0.25rem;",
            span { style: "font-size:3rem;font-weight:700;line-height:1;", "{p.value:.0}" }
            span { style: "font-size:1rem;color:#667085;", "{unit}" }
            span { style: "font-size:0.95rem;color:#667085;", "· {p.ts_label}" }
            if !p.kind.is_empty() {
                {badge(&p.kind)}
            }
            if p.food {
                {badge("🍎 food")}
            }
            if p.exercise {
                {badge("🏃 exercise")}
            }
        }
    }
}

/// Editor for the personal normal range; commits + persists on change.
fn normal_range_editor(mut settings: Signal<GlucoseSettings>, unit: String) -> Element {
    let low = settings().normal_low;
    let high = settings().normal_high;
    rsx! {
        div {
            style: "display:flex;align-items:center;gap:0.5rem;margin:0.75rem 0 0.25rem;font-size:0.9rem;color:#667085;flex-wrap:wrap;",
            span { "Normal range" }
            input {
                r#type: "number",
                min: "0",
                step: "1",
                style: "width:5rem;padding:0.2rem 0.4rem;",
                value: "{low}",
                onchange: move |e| {
                    if let Ok(v) = e.value().parse::<f64>() {
                        let mut s = settings();
                        s.normal_low = v;
                        settings.set(s);
                        glucose::save_settings(&s);
                    }
                },
            }
            span { "–" }
            input {
                r#type: "number",
                min: "0",
                step: "1",
                style: "width:5rem;padding:0.2rem 0.4rem;",
                value: "{high}",
                onchange: move |e| {
                    if let Ok(v) = e.value().parse::<f64>() {
                        let mut s = settings();
                        s.normal_high = v;
                        settings.set(s);
                        glucose::save_settings(&s);
                    }
                },
            }
            span { "{unit}" }
        }
    }
}

/// One point projected into chart pixel space, plus marker/range info we need to
/// overlay without re-deriving anything inside `rsx!`.
struct PlotPoint {
    x: f64,
    y: f64,
    /// Baseline for the food glyph (above the dot, clamped into the plot).
    food_y: f64,
    /// Baseline for the exercise glyph (above the food glyph).
    exercise_y: f64,
    /// A discrete event (manual scan / blood sample) vs. the auto sensor trace.
    is_event: bool,
    food: bool,
    exercise: bool,
    /// Dot fill — red when the reading is outside the user's normal range.
    dot_fill: &'static str,
    /// Tooltip text shown on hover (scan points).
    tip: String,
}

/// Precomputed geometry for the hover tooltip (viewBox units).
struct TooltipGeom {
    bx: f64,
    by: f64,
    tw: f64,
    th: f64,
    text_x: f64,
    text_y: f64,
    text: String,
}

/// Short date label (e.g. `Jun 20`) for a unix-second instant, or `None` if it
/// can't be represented. Used for the midnight gridlines.
fn date_label(unix_secs: i64) -> Option<String> {
    use time::macros::format_description;
    let dt = time::OffsetDateTime::from_unix_timestamp(unix_secs).ok()?;
    let fmt = format_description!("[month repr:short] [day padding:none]");
    dt.format(&fmt).ok()
}

fn glucose_chart(
    points: &[GlucosePoint],
    unit: &str,
    settings: GlucoseSettings,
    mut hovered: Signal<Option<usize>>,
) -> Element {
    // Canvas + margins (viewBox units): left holds value labels, bottom holds dates.
    let (w, h) = (760.0_f64, 200.0_f64);
    let (ml, mr, mt, mb) = (36.0_f64, 12.0_f64, 12.0_f64, 24.0_f64);
    let (px0, px1) = (ml, w - mr);
    let (py0, py1) = (mt, h - mb);
    let plot_w = px1 - px0;
    let plot_h = py1 - py0;
    let hlabel_x = px0 - 5.0;
    let date_label_y = py1 + 14.0;

    // Normal range (defensively ordered).
    let (low, high) = if settings.normal_low <= settings.normal_high {
        (settings.normal_low, settings.normal_high)
    } else {
        (settings.normal_high, settings.normal_low)
    };

    // Data + time extents.
    let mut vmin = f64::INFINITY;
    let mut vmax = f64::NEG_INFINITY;
    let mut tmin = i64::MAX;
    let mut tmax = i64::MIN;
    for p in points {
        vmin = vmin.min(p.value);
        vmax = vmax.max(p.value);
        tmin = tmin.min(p.ts_unix);
        tmax = tmax.max(p.ts_unix);
    }

    // Value domain rounded out to 50s, widened to include the normal band so it's
    // always visible; guard against a degenerate span.
    let y_min = ((vmin.min(low) / 50.0).floor() * 50.0).max(0.0);
    let mut y_max = (vmax.max(high) / 50.0).ceil() * 50.0;
    if y_max <= y_min {
        y_max = y_min + 50.0;
    }
    let y_span = y_max - y_min;
    let trange = if tmax <= tmin { 1 } else { tmax - tmin };

    let map_y = |v: f64| py0 + (1.0 - (v - y_min) / y_span) * plot_h;
    let map_x = |t: i64| px0 + (t - tmin) as f64 / trange as f64 * plot_w;

    // Horizontal gridlines at each 50: (value, line y, label y).
    let mut hlines: Vec<(f64, f64, f64)> = Vec::new();
    let mut gv = y_min;
    while gv <= y_max + 0.5 {
        let y = map_y(gv);
        hlines.push((gv, y, y + 3.0));
        gv += 50.0;
    }

    // Vertical gridlines at each local midnight — ts_unix is local wall-clock as if
    // UTC, so local midnight is a multiple of 86_400. Thin labels if many days.
    let mut midnights: Vec<i64> = Vec::new();
    let mut mid = tmin.div_euclid(86_400) * 86_400;
    if mid < tmin {
        mid += 86_400;
    }
    while mid <= tmax {
        midnights.push(mid);
        mid += 86_400;
    }
    let stride = ((midnights.len() as f64) / 12.0).ceil().max(1.0) as usize;
    let vlines: Vec<(f64, Option<String>)> = midnights
        .iter()
        .enumerate()
        .map(|(i, &t)| {
            let label = if i % stride == 0 { date_label(t) } else { None };
            (map_x(t), label)
        })
        .collect();

    // Out-of-range bands: (y, height) — above high, and below low.
    let band_above = (high < y_max).then(|| (py0, map_y(high) - py0));
    let band_below = (low > y_min).then(|| (map_y(low), py1 - map_y(low)));

    let plotted: Vec<PlotPoint> = points
        .iter()
        .map(|p| {
            let y = map_y(p.value);
            let food_y = (y - 9.0).clamp(py0 + 3.0, py1);
            PlotPoint {
                x: map_x(p.ts_unix),
                y,
                food_y,
                exercise_y: (food_y - 14.0).clamp(py0 + 1.0, py1),
                is_event: p.kind == "scan" || p.kind == "blood",
                food: p.food,
                exercise: p.exercise,
                dot_fill: if p.value < low || p.value > high {
                    "#f87171"
                } else {
                    "#9db8f0"
                },
                tip: format!("{:.0} {} · {}", p.value, unit, p.ts_label),
            }
        })
        .collect();
    let poly = plotted
        .iter()
        .map(|pp| format!("{:.1},{:.1}", pp.x, pp.y))
        .collect::<Vec<_>>()
        .join(" ");

    // Tooltip geometry for the currently-hovered scan point (if any). Positioned
    // above the dot, flipped below near the top edge, and clamped within the plot.
    let tooltip = hovered().and_then(|idx| plotted.get(idx)).map(|pp| {
        let tw = (pp.tip.chars().count() as f64) * 5.6 + 12.0;
        let th = 18.0;
        let bx = (pp.x - tw / 2.0).clamp(px0, (px1 - tw).max(px0));
        let by = if pp.y - th - 8.0 >= py0 {
            pp.y - th - 8.0
        } else {
            pp.y + 8.0
        };
        TooltipGeom {
            bx,
            by,
            tw,
            th,
            text_x: bx + tw / 2.0,
            text_y: by + 12.5,
            text: pp.tip.clone(),
        }
    });

    rsx! {
        div { style: "margin:0.5rem 0 1rem;",
            svg {
                width: "100%",
                height: "{h}",
                view_box: "0 0 {w} {h}",

                // Out-of-range bands (behind everything).
                if let Some((y, ht)) = band_above {
                    rect {
                        x: "{px0}", y: "{y}", width: "{plot_w}", height: "{ht}",
                        style: "fill:#ef4444;fill-opacity:0.13;",
                    }
                }
                if let Some((y, ht)) = band_below {
                    rect {
                        x: "{px0}", y: "{y}", width: "{plot_w}", height: "{ht}",
                        style: "fill:#ef4444;fill-opacity:0.13;",
                    }
                }

                // Horizontal gridlines + value labels.
                for (i , (val , y , label_y)) in hlines.iter().enumerate() {
                    g { key: "h{i}",
                        line {
                            x1: "{px0}", y1: "{y}", x2: "{px1}", y2: "{y}",
                            style: "stroke:rgba(255,255,255,0.08);stroke-width:1;",
                        }
                        text {
                            x: "{hlabel_x}", y: "{label_y}",
                            style: "text-anchor:end;font-size:10px;fill:#667085;",
                            "{val:.0}"
                        }
                    }
                }

                // Vertical midnight gridlines + date labels.
                for (i , (x , label)) in vlines.iter().enumerate() {
                    g { key: "v{i}",
                        line {
                            x1: "{x}", y1: "{py0}", x2: "{x}", y2: "{py1}",
                            style: "stroke:rgba(255,255,255,0.10);stroke-width:1;",
                        }
                        if let Some(text_label) = label {
                            text {
                                x: "{x}", y: "{date_label_y}",
                                style: "text-anchor:middle;font-size:10px;fill:#98a2b3;",
                                "{text_label}"
                            }
                        }
                    }
                }

                // The glucose trace.
                polyline {
                    points: "{poly}",
                    fill: "none",
                    stroke: "#5b8def",
                    stroke_width: "2",
                    stroke_linejoin: "round",
                }

                // Scan markers + food/exercise glyphs (out-of-range dots in red).
                for (i , pp) in plotted.iter().enumerate() {
                    if pp.is_event {
                        g { key: "ev{i}",
                            circle {
                                cx: "{pp.x}", cy: "{pp.y}", r: "3.5",
                                fill: "{pp.dot_fill}", stroke: "#0b0f1a", stroke_width: "1.5",
                            }
                            if pp.food {
                                text {
                                    x: "{pp.x}", y: "{pp.food_y}",
                                    style: "text-anchor:middle;font-size:13px;",
                                    "🍎"
                                }
                            }
                            if pp.exercise {
                                text {
                                    x: "{pp.x}", y: "{pp.exercise_y}",
                                    style: "text-anchor:middle;font-size:13px;",
                                    "🏃"
                                }
                            }
                            // Invisible, larger hit area so the small dot is easy to hover.
                            circle {
                                cx: "{pp.x}", cy: "{pp.y}", r: "8",
                                style: "fill:transparent;cursor:pointer;",
                                onmouseenter: move |_| hovered.set(Some(i)),
                                onmouseleave: move |_| hovered.set(None),
                            }
                        }
                    }
                }

                // Hover tooltip (drawn last so it sits on top).
                if let Some(t) = &tooltip {
                    g {
                        rect {
                            x: "{t.bx}", y: "{t.by}", width: "{t.tw}", height: "{t.th}",
                            rx: "3", ry: "3",
                            style: "fill:#1b2230;stroke:rgba(255,255,255,0.18);stroke-width:1;",
                        }
                        text {
                            x: "{t.text_x}", y: "{t.text_y}",
                            style: "text-anchor:middle;font-size:11px;fill:#e6eaf2;",
                            "{t.text}"
                        }
                    }
                }
            }

            // Legend + actual data extents.
            div {
                style: "margin-top:0.35rem;font-size:0.8rem;color:#98a2b3;display:flex;gap:1rem;flex-wrap:wrap;align-items:center;",
                span { "range {vmin:.0}–{vmax:.0} {unit}" }
                span {
                    span { style: "color:#9db8f0;", "●" }
                    " scan"
                }
                span { "🍎 food" }
                span { "🏃 exercise" }
                span {
                    span { style: "display:inline-block;width:0.8rem;height:0.8rem;background:#ef4444;opacity:0.30;border-radius:2px;vertical-align:middle;margin-right:0.2rem;" }
                    "out of range"
                }
            }
        }
    }
}

fn recent_list(points: &[GlucosePoint], unit: &str) -> Element {
    rsx! {
        div { style: "border-top:1px solid #e4e7ec;margin-top:0.5rem;",
            for (i , p) in points.iter().rev().take(12).enumerate() {
                div {
                    key: "{i}",
                    style: "display:flex;justify-content:space-between;gap:1rem;padding:0.35rem 0;border-bottom:1px solid #f2f4f7;font-size:0.9rem;",
                    span { style: "color:#475467;", "{p.ts_label}" }
                    span { style: "font-weight:600;", "{p.value:.0} {unit}" }
                    span { style: "color:#98a2b3;min-width:4rem;text-align:right;", "{p.kind}" }
                }
            }
        }
    }
}

// ---- Sync button: desktop wires the reader; web/mobile renders nothing -----

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn sync_action(mut data: Signal<GlucoseData>, mut status: Signal<SyncStatus>) -> Element {
    let running = matches!(&*status.read(), SyncStatus::Running);
    let onclick = move |_| {
        if matches!(&*status.peek(), SyncStatus::Running) {
            return;
        }
        status.set(SyncStatus::Running);
        // All device I/O runs on the shared, long-lived device thread (macOS pins
        // hidapi to one CFRunLoop); we just await its result on the UI task.
        spawn(async move {
            match glucose::request_sync().await {
                Ok(Ok(report)) => {
                    data.set(glucose::load());
                    status.set(SyncStatus::Done {
                        serial: report.serial,
                        added: report.added,
                        total: report.total,
                    });
                }
                Ok(Err(e)) => status.set(SyncStatus::Error(e)),
                Err(_) => status.set(SyncStatus::Error("device thread unavailable".into())),
            }
        });
    };
    rsx! {
        button {
            r#type: "button",
            class: "button button--primary",
            disabled: running,
            onclick: onclick,
            if running {
                {crate::t!("glucose-syncing")}
            } else {
                {crate::t!("glucose-sync")}
            }
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn sync_action(_data: Signal<GlucoseData>, _status: Signal<SyncStatus>) -> Element {
    rsx! {}
}

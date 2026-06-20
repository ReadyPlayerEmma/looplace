//! The in-app Glucose (health) view: latest reading, a sparkline trend, and a
//! recent-readings list.
//!
//! On desktop, "Sync from reader" pulls the FreeStyle Libre 2 over USB on a
//! background thread (so the UI never freezes) and writes the local store. On
//! web/mobile the view shows a desktop-only note instead of a sync button.

use dioxus::prelude::*;

use crate::core::glucose::{self, GlucoseData, GlucosePoint};

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
            if count >= 2 {
                {sparkline(&snapshot.points, &snapshot.unit)}
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

/// One point projected into the sparkline's pixel space, plus the marker info we
/// need to overlay scan events without re-deriving anything inside `rsx!`.
struct PlotPoint {
    x: f64,
    y: f64,
    /// Baseline for the food glyph (above the dot, clamped into view).
    food_y: f64,
    /// Baseline for the exercise glyph (above the food glyph).
    exercise_y: f64,
    /// A discrete event (manual scan / blood sample) vs. the auto sensor trace.
    is_event: bool,
    food: bool,
    exercise: bool,
}

fn sparkline(points: &[GlucosePoint], unit: &str) -> Element {
    let (w, h, pad) = (720.0_f64, 160.0_f64, 10.0_f64);

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
    // Guard against zero ranges (all-equal values or a single timestamp).
    let vrange = if (vmax - vmin) < 1.0 { 1.0 } else { vmax - vmin };
    let trange = if tmax <= tmin { 1 } else { tmax - tmin };

    let plotted: Vec<PlotPoint> = points
        .iter()
        .map(|p| {
            let x = pad + (p.ts_unix - tmin) as f64 / trange as f64 * (w - 2.0 * pad);
            let y = pad + (1.0 - (p.value - vmin) / vrange) * (h - 2.0 * pad);
            let food_y = (y - 9.0).clamp(13.0, h - 4.0);
            PlotPoint {
                x,
                y,
                food_y,
                exercise_y: (food_y - 14.0).clamp(11.0, h - 4.0),
                is_event: p.kind == "scan" || p.kind == "blood",
                food: p.food,
                exercise: p.exercise,
            }
        })
        .collect();
    let poly = plotted
        .iter()
        .map(|pp| format!("{:.1},{:.1}", pp.x, pp.y))
        .collect::<Vec<_>>()
        .join(" ");

    rsx! {
        div { style: "margin:0.5rem 0 1rem;",
            svg {
                width: "100%",
                height: "{h}",
                view_box: "0 0 {w} {h}",
                // Continuous sensor + scan trace.
                polyline {
                    points: "{poly}",
                    fill: "none",
                    stroke: "#5b8def",
                    stroke_width: "2",
                    stroke_linejoin: "round",
                }
                // Overlay a marker on each discrete scan, with food/exercise glyphs.
                for (i , pp) in plotted.iter().enumerate() {
                    if pp.is_event {
                        g { key: "ev{i}",
                            circle {
                                cx: "{pp.x}",
                                cy: "{pp.y}",
                                r: "3.5",
                                fill: "#9db8f0",
                                stroke: "#0b0f1a",
                                stroke_width: "1.5",
                            }
                            if pp.food {
                                text {
                                    x: "{pp.x}",
                                    y: "{pp.food_y}",
                                    style: "text-anchor:middle;font-size:13px;",
                                    "🍎"
                                }
                            }
                            if pp.exercise {
                                text {
                                    x: "{pp.x}",
                                    y: "{pp.exercise_y}",
                                    style: "text-anchor:middle;font-size:13px;",
                                    "🏃"
                                }
                            }
                        }
                    }
                }
            }
            div {
                style: "display:flex;justify-content:space-between;font-size:0.8rem;color:#667085;",
                span { "min {vmin:.0} {unit}" }
                span { "max {vmax:.0} {unit}" }
            }
            div {
                style: "margin-top:0.35rem;font-size:0.8rem;color:#98a2b3;display:flex;gap:1rem;flex-wrap:wrap;",
                span {
                    span { style: "color:#9db8f0;", "●" }
                    " scan"
                }
                span { "🍎 food" }
                span { "🏃 exercise" }
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

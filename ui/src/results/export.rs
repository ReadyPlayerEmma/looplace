use base64::Engine as _;
use dioxus::prelude::*;
use std::fmt::Write as _;

#[cfg(target_arch = "wasm32")]
use crate::core::platform;
use crate::core::{format, storage::SummaryRecord};
use crate::results::{
    format_date_badge, format_time_badge, format_timestamp, parse_nback_metrics, parse_pvt_metrics,
    parse_timestamp, qc_summary, record_is_clean,
};
use time::OffsetDateTime;

const EXPORT_CANVAS_WIDTH: f64 = 1200.0;
const EXPORT_CANVAS_HEIGHT: f64 = 900.0;
const CANVAS_MARGIN: f64 = 64.0;
const CANVAS_GUTTER: f64 = 28.0;
const CARD_RADIUS: f64 = 24.0;
const CARD_PADDING_X: f64 = 32.0;
const CARD_PADDING_Y: f64 = 28.0;
const HIGHLIGHT_GAP: f64 = 20.0;
const MIN_SPARK_PLOT_HEIGHT: f64 = 80.0;
const MIN_BARS_PLOT_HEIGHT: f64 = 70.0;
const MAX_SPARK_PLOT_HEIGHT: f64 = 180.0;
const MAX_BARS_PLOT_HEIGHT: f64 = 130.0;

// Grid / stroke opacities
const OP_GRID: f64 = 0.10;

// Snapshot-scoped ID generator + pixel snapping helpers
struct IdGen {
    counter: u64,
    prefix: String,
}
impl IdGen {
    fn new(prefix: &str) -> Self {
        Self {
            counter: 1,
            prefix: prefix.to_string(),
        }
    }
    fn next(&mut self, stem: &str) -> String {
        let id = self.counter;
        self.counter += 1;
        format!("{}-{}-{id}", self.prefix, stem)
    }
}
fn crisp(v: f64) -> f64 {
    v.floor() + 0.5
}

/// Ellipsize helper for bar labels (defensive)
fn ellipsize(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let mut it = s.chars();
        let truncated: String = it.by_ref().take(max_chars.saturating_sub(1)).collect();
        format!("{truncated}…")
    }
}

// Opacity tokens (separated from base hex colors)
const OP_CARD_FILL: f64 = 0.05;
const OP_CARD_STROKE: f64 = 0.08;
const OP_TEXT_MUTED: f64 = 0.62;
const OP_TEXT_META: f64 = 0.55;
const OP_ACCENT_SOFT: f64 = 0.12;
const FONT_STACK: &str = "Inter, Segoe UI, sans-serif";
static INTER_B64: once_cell::sync::Lazy<String> = once_cell::sync::Lazy::new(|| {
    base64::engine::general_purpose::STANDARD
        .encode(include_bytes!("../../assets/Inter-Variable.ttf"))
});

#[derive(Clone, Copy)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Rect {
    fn inset(&self, x: f64, y: f64) -> Rect {
        Rect {
            x: self.x + x,
            y: self.y + y,
            width: (self.width - 2.0 * x).max(0.0),
            height: (self.height - 2.0 * y).max(0.0),
        }
    }
}

struct Layout {
    gutter: f64,
    cursor_y: f64,
    content_x: f64,
    content_width: f64,
}

impl Layout {
    fn new(total_width: f64, margin: f64, gutter: f64) -> Self {
        Self {
            gutter,
            cursor_y: margin,
            content_x: margin,
            content_width: total_width - margin * 2.0,
        }
    }

    fn place_block(&mut self, height: f64) -> Rect {
        let rect = Rect {
            x: self.content_x,
            y: self.cursor_y,
            width: self.content_width,
            height,
        };
        self.cursor_y += height + self.gutter;
        rect
    }

    fn report(&self, canvas_h: f64) -> LayoutReport {
        LayoutReport {
            used_height: self.cursor_y - self.gutter + CANVAS_MARGIN,
            canvas_height: canvas_h,
        }
    }
}

struct LayoutReport {
    used_height: f64,
    canvas_height: f64,
}

#[derive(Clone, Copy)]
struct TextMetrics {
    line_height: f64,
    baseline: f64,
}

fn text_metrics(size: f64, #[allow(unused_variables)] weight: &str) -> TextMetrics {
    #[cfg(feature = "embed_inter")]
    {
        use crate::results::fonts::{measure, FontWeight};
        let wt = match weight {
            "bold" => FontWeight::Bold,
            "semibold" | "semi" => FontWeight::SemiBold,
            _ => FontWeight::Regular,
        };
        let m = measure(wt, size);
        TextMetrics {
            line_height: m.line_h,
            baseline: m.asc,
        }
    }
    #[cfg(not(feature = "embed_inter"))]
    {
        // Mark weight as used so non-embed builds don’t warn.
        let _ = weight;
        let line_height = size * 1.28;
        let baseline = size * 0.92;
        TextMetrics {
            line_height,
            baseline,
        }
    }
}

// Separate color from opacity
struct Theme {
    background: &'static str,
    card_fill: &'static str,
    card_stroke: &'static str,
    text_primary: &'static str,
    text_base: &'static str,
    accent: &'static str,
}

impl Theme {
    fn default() -> Self {
        Self {
            background: "url(#bg)",
            card_fill: "#ffffff",
            card_stroke: "#ffffff",
            text_primary: "#f5f7fb",
            text_base: "#f5f7fb",
            accent: "#f05a7e",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum ExportStatus {
    Idle,
    Working(&'static str),
    Done(String),
    Error(String),
}

#[component]
pub fn ResultsExportPanel(records: Vec<SummaryRecord>) -> Element {
    let total_runs = records.len();
    let pvt_runs = records.iter().filter(|r| r.task == "pvt").count();
    let nback_runs = records.iter().filter(|r| r.task == "nback2").count();

    let status = use_signal(|| ExportStatus::Idle);
    let busy = use_signal(|| false);

    let feedback = match &status() {
        ExportStatus::Idle => None,
        ExportStatus::Working(label) => {
            Some(("results-card__meta".to_string(), format!("{label}…")))
        }
        ExportStatus::Done(message) => Some((
            "results-card__meta results-card__meta--success".to_string(),
            format!("✅ {message}"),
        )),
        ExportStatus::Error(err) => Some((
            "results-card__meta results-card__meta--error".to_string(),
            format!("⚠️ {err}"),
        )),
    };

    let json_handler = {
        let export_records = records.clone();
        let mut status_signal = status;
        let mut busy_signal = busy;
        move |_| {
            if busy_signal() {
                return;
            }
            busy_signal.set(true);
            status_signal.set(ExportStatus::Working("Preparing JSON"));
            let export_records = export_records.clone();
            #[cfg(target_arch = "wasm32")]
            {
                let status_signal = status_signal;
                let busy_signal = busy_signal;
                platform::spawn_future(async move {
                    let outcome = perform_json_export(export_records).await;
                    match outcome {
                        Ok(message) => status_signal.set(ExportStatus::Done(message)),
                        Err(err) => status_signal.set(ExportStatus::Error(err)),
                    }
                    busy_signal.set(false);
                });
            }

            #[cfg(not(target_arch = "wasm32"))]
            {
                let outcome = futures::executor::block_on(perform_json_export(export_records));
                match outcome {
                    Ok(message) => status_signal.set(ExportStatus::Done(message)),
                    Err(err) => status_signal.set(ExportStatus::Error(err)),
                }
                busy_signal.set(false);
            }
        }
    };

    let csv_handler = {
        let export_records = records.clone();
        let mut status_signal = status;
        let mut busy_signal = busy;
        move |_| {
            if busy_signal() {
                return;
            }
            busy_signal.set(true);
            status_signal.set(ExportStatus::Working("Preparing CSV"));
            let export_records = export_records.clone();
            #[cfg(target_arch = "wasm32")]
            {
                let status_signal = status_signal;
                let busy_signal = busy_signal;
                platform::spawn_future(async move {
                    let outcome = perform_csv_export(export_records).await;
                    match outcome {
                        Ok(message) => status_signal.set(ExportStatus::Done(message)),
                        Err(err) => status_signal.set(ExportStatus::Error(err)),
                    }
                    busy_signal.set(false);
                });
            }

            #[cfg(not(target_arch = "wasm32"))]
            {
                let outcome = futures::executor::block_on(perform_csv_export(export_records));
                match outcome {
                    Ok(message) => status_signal.set(ExportStatus::Done(message)),
                    Err(err) => status_signal.set(ExportStatus::Error(err)),
                }
                busy_signal.set(false);
            }
        }
    };

    let png_handler = {
        let export_records = records.clone();
        let mut status_signal = status;
        let mut busy_signal = busy;
        move |_| {
            if busy_signal() {
                return;
            }
            busy_signal.set(true);
            status_signal.set(ExportStatus::Working("Preparing PNG"));
            let export_records = export_records.clone();
            #[cfg(target_arch = "wasm32")]
            {
                let status_signal = status_signal;
                let busy_signal = busy_signal;
                platform::spawn_future(async move {
                    let outcome = perform_png_export(export_records).await;
                    match outcome {
                        Ok(message) => status_signal.set(ExportStatus::Done(message)),
                        Err(err) => status_signal.set(ExportStatus::Error(err)),
                    }
                    busy_signal.set(false);
                });
            }

            #[cfg(not(target_arch = "wasm32"))]
            {
                let outcome = futures::executor::block_on(perform_png_export(export_records));
                match outcome {
                    Ok(message) => status_signal.set(ExportStatus::Done(message)),
                    Err(err) => status_signal.set(ExportStatus::Error(err)),
                }
                busy_signal.set(false);
            }
        }
    };

    rsx! {
        section { class: "results-card results-export",
            div { class: "results-card__header",
                h2 { "Export" }
            }

            if total_runs == 0 {
                p { class: "results-card__placeholder", "Exports unlock once runs are stored locally." }
            } else {
                p {
                    "Prepare tidy JSON, CSV, or PNG captures for deeper analysis and sharing."
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
                        disabled: busy(),
                        onclick: json_handler,
                        "Export JSON"
                    }
                    button {
                        r#type: "button",
                        class: "button",
                        disabled: busy(),
                        onclick: csv_handler,
                        "Export CSV"
                    }
                    button {
                        r#type: "button",
                        class: "button button--ghost",
                        disabled: busy(),
                        onclick: png_handler,
                        "Export PNG"
                    }
                }

                if let Some((class_name, message)) = feedback {
                    p { class: "{class_name}", "{message}" }
                }
            }
        }
    }
}

async fn perform_json_export(records: Vec<SummaryRecord>) -> Result<String, String> {
    let json = serde_json::to_string_pretty(&records).map_err(|err| err.to_string())?;
    copy_to_clipboard(json.clone()).await?;
    let filename = format!("looplace-results-{}.json", timestamp_slug());
    let delivery = download_bytes(&filename, "application/json", json.into_bytes()).await?;
    Ok(match delivery {
        Some(path) => format!("JSON copied and saved to {path}"),
        None => "JSON copied to clipboard and download started".to_string(),
    })
}

async fn perform_csv_export(records: Vec<SummaryRecord>) -> Result<String, String> {
    let csv = build_csv(&records);
    let filename = format!("looplace-results-{}.csv", timestamp_slug());
    let delivery = download_bytes(&filename, "text/csv", csv.into_bytes()).await?;
    Ok(match delivery {
        Some(path) => format!("CSV saved to {path}"),
        None => "CSV download started".to_string(),
    })
}

async fn perform_png_export(records: Vec<SummaryRecord>) -> Result<String, String> {
    let png_bytes = build_png_snapshot(&records).await?;
    let filename = format!("looplace-results-{}.png", timestamp_slug());
    let delivery = download_bytes(&filename, "image/png", png_bytes).await?;
    Ok(match delivery {
        Some(path) => format!("PNG snapshot saved to {path}"),
        None => "PNG download started".to_string(),
    })
}

fn build_csv(records: &[SummaryRecord]) -> String {
    // Fixed schema: 4 core + 5 PVT + 7 NBack + 5 tail = 21 columns
    let header = [
        "task",
        "created_at",
        "platform",
        "tz",
        // PVT metrics
        "median_rt_ms",
        "mean_rt_ms",
        "lapses_500ms",
        "false_starts",
        "slope_ms_per_min",
        // N-back metrics
        "accuracy",
        "d_prime",
        "criterion",
        "hits",
        "misses",
        "false_alarms",
        "correct_rejections",
        // General / QC
        "notes",
        "qc_summary",
        "qc_focus_lost",
        "qc_visibility_blur",
        "qc_min_trials_met",
    ];
    let mut out = String::new();
    out.push_str(
        &header
            .iter()
            .map(|c| c.to_string())
            .map(|f| escape_csv(&f))
            .collect::<Vec<_>>()
            .join(","),
    );
    out.push('\n');

    for record in records {
        let mut row: Vec<String> = Vec::with_capacity(header.len());

        // Core
        row.push(record.task.clone());
        row.push(record.created_at.clone());
        row.push(record.client.platform.clone());
        row.push(record.client.tz.clone());

        // PVT (5)
        if record.task == "pvt" {
            if let Some(m) = parse_pvt_metrics(record) {
                row.extend([
                    m.median_rt_ms.to_string(),
                    m.mean_rt_ms.to_string(),
                    m.lapses_ge_500ms.to_string(),
                    m.false_starts.to_string(),
                    m.time_on_task_slope_ms_per_min.to_string(),
                ]);
            } else {
                row.extend(std::iter::repeat(String::new()).take(5));
            }
        } else {
            row.extend(std::iter::repeat(String::new()).take(5));
        }

        // NBack (7)
        if record.task == "nback2" {
            if let Some(m) = parse_nback_metrics(record) {
                row.extend([
                    m.accuracy.to_string(),
                    m.d_prime.to_string(),
                    m.criterion.to_string(),
                    m.hits.to_string(),
                    m.misses.to_string(),
                    m.false_alarms.to_string(),
                    m.correct_rejections.to_string(),
                ]);
            } else {
                row.extend(std::iter::repeat(String::new()).take(7));
            }
        } else {
            row.extend(std::iter::repeat(String::new()).take(7));
        }

        // Tail (5)
        row.push(record.notes.clone().unwrap_or_default());
        row.push(qc_summary(record));
        row.push(record.qc.focus_lost_events.to_string());
        row.push(record.qc.visibility_blur_events.to_string());
        row.push(record.qc.min_trials_met.to_string());

        out.push_str(
            &row.into_iter()
                .map(|f| escape_csv(&f))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push('\n');
    }
    out
}

fn escape_csv(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    let needs_quotes = value.contains(',') || value.contains('"') || value.contains('\n');
    if needs_quotes {
        let escaped = value.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        value.to_string()
    }
}

fn timestamp_slug() -> String {
    use time::{macros::format_description, OffsetDateTime};
    OffsetDateTime::now_utc()
        .format(&format_description!(
            "[year][month][day]_[hour][minute][second]"
        ))
        .unwrap_or_else(|_| "export".into())
}

async fn copy_to_clipboard(payload: String) -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().ok_or("window unavailable")?;
        let document = window.document().ok_or("document unavailable")?;
        let body = document.body().ok_or("missing body")?;
        let textarea = document
            .create_element("textarea")
            .map_err(|_| "Unable to create textarea")?
            .dyn_into::<web_sys::HtmlTextAreaElement>()
            .map_err(|_| "Textarea cast failed")?;
        textarea.set_value(&payload);
        let style = textarea.style();
        style.set_property("position", "fixed").ok();
        style.set_property("top", "0").ok();
        style.set_property("left", "0").ok();
        style.set_property("opacity", "0").ok();
        body.append_child(&textarea).ok();
        textarea.select();
        if !document.exec_command("copy").unwrap_or(false) {
            textarea.remove();
            return Err("Clipboard copy blocked".into());
        }
        textarea.remove();
        Ok(())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use arboard::Clipboard;
        let mut clipboard = Clipboard::new().map_err(|err| err.to_string())?;
        clipboard.set_text(payload).map_err(|err| err.to_string())
    }
}

async fn download_bytes(
    filename: &str,
    mime: &str,
    bytes: Vec<u8>,
) -> Result<Option<String>, String> {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url};
        let array = js_sys::Uint8Array::from(bytes.as_slice());
        let mut parts = js_sys::Array::new();
        parts.push(&array.buffer());
        let mut opts = BlobPropertyBag::new();
        opts.type_(mime);
        let blob = Blob::new_with_u8_array_sequence_and_options(&parts, &opts)
            .map_err(|_| "Failed to create blob".to_string())?;
        let url = Url::create_object_url_with_blob(&blob)
            .map_err(|_| "Unable to create download".to_string())?;
        let document = web_sys::window()
            .and_then(|w| w.document())
            .ok_or("Document unavailable")?;
        let anchor: HtmlAnchorElement = document
            .create_element("a")
            .map_err(|_| "Unable to create anchor")?
            .dyn_into()
            .map_err(|_| "Anchor cast failed")?;
        anchor.set_href(&url);
        anchor.set_download(filename);
        anchor.style().set_property("display", "none").ok();
        document
            .body()
            .ok_or("Missing body")?
            .append_child(&anchor)
            .ok();
        anchor.click();
        anchor.remove();
        Url::revoke_object_url(&url).ok();
        Ok(None)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::fs;
        use std::io::Write;
        let _ = mime;
        let dir = desktop_export_dir()?;
        fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        let path = dir.join(filename);
        let mut file = fs::File::create(&path).map_err(|err| err.to_string())?;
        file.write_all(&bytes).map_err(|err| err.to_string())?;
        Ok(Some(path.to_string_lossy().to_string()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn desktop_export_dir() -> Result<std::path::PathBuf, String> {
    let dirs = directories::ProjectDirs::from("com", "Looplace", "Looplace")
        .ok_or("Unable to determine export directory")?;
    Ok(dirs.data_dir().join("exports"))
}

async fn build_png_snapshot(records: &[SummaryRecord]) -> Result<Vec<u8>, String> {
    #[cfg(target_arch = "wasm32")]
    {
        build_png_web(records).await
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        build_png_desktop(records)
    }
}

#[cfg(target_arch = "wasm32")]
async fn build_png_web(records: &[SummaryRecord]) -> Result<Vec<u8>, String> {
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{window, Blob, BlobPropertyBag, HtmlCanvasElement, Url};

    let svg_markup = svg_snapshot(records);

    // Build an SVG blob URL
    let mut opts = BlobPropertyBag::new();
    opts.type_("image/svg+xml");
    let arr = js_sys::Array::of1(&JsValue::from_str(&svg_markup));
    let blob = Blob::new_with_str_sequence_and_options(&arr, &opts)
        .map_err(|_| "Unable to build SVG blob")?;
    let url = Url::create_object_url_with_blob(&blob).map_err(|_| "Unable to create SVG URL")?;

    let win = window().ok_or("window unavailable")?;
    let dpr = win.device_pixel_ratio();
    let doc = win.document().ok_or("document unavailable")?;

    // Canvas sized for device pixel ratio
    let canvas: HtmlCanvasElement = doc
        .create_element("canvas")
        .map_err(|_| "Create canvas failed")?
        .dyn_into()
        .map_err(|_| "Canvas cast failed")?;
    canvas.set_width((EXPORT_CANVAS_WIDTH * dpr) as u32);
    canvas.set_height((EXPORT_CANVAS_HEIGHT * dpr) as u32);

    // Decode the SVG to an ImageBitmap (faster, avoids layout)
    let bitmap_promise = win
        .create_image_bitmap_with_src(&JsValue::from_str(&url))
        .map_err(|_| "createImageBitmap unsupported")?;
    let bitmap = JsFuture::from(bitmap_promise)
        .await
        .map_err(|_| "Bitmap decode failed")?;

    let ctx = canvas
        .get_context("2d")
        .map_err(|_| "2D context unavailable")?
        .ok_or("2D context missing")?
        .dyn_into::<web_sys::CanvasRenderingContext2d>()
        .map_err(|_| "Context cast failed")?;
    ctx.scale(dpr, dpr).map_err(|_| "Scale failed")?;
    let _ = ctx.draw_image_with_image_bitmap(&bitmap.unchecked_into(), 0.0, 0.0);

    let data_url = canvas
        .to_data_url_with_type("image/png")
        .map_err(|_| "Canvas toDataURL failed")?;
    Url::revoke_object_url(&url).ok();

    let b64 = data_url.split(',').nth(1).ok_or("Malformed data URL")?;
    base64::decode(b64).map_err(|_| "PNG decode failed")
}

#[cfg(not(target_arch = "wasm32"))]
fn build_png_desktop(records: &[SummaryRecord]) -> Result<Vec<u8>, String> {
    // Allow environment override for scale (e.g. LOOPLACE_EXPORT_SCALE=2)
    let scale = std::env::var("LOOPLACE_EXPORT_SCALE")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|s| *s >= 1.0 && *s <= 4.0)
        .unwrap_or(1.5);
    let svg_markup = svg_snapshot_with_scale(records, scale);
    let w = (EXPORT_CANVAS_WIDTH * scale).round() as u32;
    let h = (EXPORT_CANVAS_HEIGHT * scale).round() as u32;
    svg_to_png(&svg_markup, w, h)
}

fn fit_plots(
    spark: &mut f64,
    bars: &mut f64,
    min_spark: f64,
    min_bars: f64,
    avail_height: f64,
    static_height: f64,
) {
    let target = (avail_height - static_height).max(min_spark + min_bars);
    let mut current = *spark + *bars;
    if current <= target {
        // Still clamp to maximums for safety
        *spark = spark.min(MAX_SPARK_PLOT_HEIGHT);
        *bars = bars.min(MAX_BARS_PLOT_HEIGHT);
        return;
    }

    loop {
        let flex_s = (*spark - min_spark).max(0.0);
        let flex_b = (*bars - min_bars).max(0.0);
        let flex = flex_s + flex_b;
        if flex <= 1e-6 {
            break;
        }

        let need = current - target;
        let k = (need / flex).min(1.0);

        *spark -= flex_s * k;
        *bars -= flex_b * k;
        current = *spark + *bars;

        if current <= target + 1e-6 {
            break;
        }
    }

    // Guard rails + ceilings
    if *spark < min_spark {
        *spark = min_spark;
    }
    if *bars < min_bars {
        *bars = min_bars;
    }
    *spark = spark.min(MAX_SPARK_PLOT_HEIGHT);
    *bars = bars.min(MAX_BARS_PLOT_HEIGHT);
}

pub fn svg_snapshot(records: &[SummaryRecord]) -> String {
    svg_snapshot_with_scale(records, 1.0)
}

#[allow(dead_code)]
const _SVG_SNAPSHOT_REF: fn(&[SummaryRecord]) -> String = svg_snapshot;

fn svg_snapshot_with_scale(records: &[SummaryRecord], scale: f64) -> String {
    let overview = SnapshotOverview::build(records);
    let theme = Theme::default();

    let title_metrics = text_metrics(56.0, "bold");
    let subtitle_metrics = text_metrics(26.0, "regular");
    let meta_metrics = text_metrics(18.0, "regular");

    let mut subtitle = match overview.total_runs {
        0 => "No runs saved yet".to_string(),
        1 => "1 run saved locally".to_string(),
        n => format!("{n} runs saved locally"),
    };
    if overview.total_runs > 0 {
        if overview.clean_runs == overview.total_runs && overview.clean_runs > 0 {
            subtitle.push_str(" · all clean");
        } else if overview.clean_runs > 0 {
            subtitle.push_str(&format!(" · {} clean", overview.clean_runs));
        }
    }

    let latest_line = overview
        .latest_label
        .as_ref()
        .map(|label| format!("Latest run {label}"));

    let mut title_block_height = title_metrics.line_height + subtitle_metrics.line_height + 10.0;
    if latest_line.is_some() {
        title_block_height += meta_metrics.line_height + 8.0;
    }

    let highlight_label_metrics = text_metrics(16.0, "regular");
    let highlight_value_metrics = text_metrics(36.0, "semibold");
    let highlight_meta_metrics = text_metrics(15.0, "regular");
    let highlight_block_height = CARD_PADDING_Y
        + highlight_label_metrics.line_height
        + 6.0
        + highlight_value_metrics.line_height
        + 8.0
        + highlight_meta_metrics.line_height
        + CARD_PADDING_Y / 2.0;

    let spark_header_metrics = (
        text_metrics(24.0, "semibold"),
        text_metrics(16.0, "regular"),
    );
    let mut spark_plot_height = 120.0;
    let spark_header_height =
        spark_header_metrics.0.line_height + 6.0 + spark_header_metrics.1.line_height;

    let bars_header_metrics = (
        text_metrics(24.0, "semibold"),
        text_metrics(16.0, "regular"),
    );
    let mut bars_plot_height = 90.0;
    let bars_header_height =
        bars_header_metrics.0.line_height + 6.0 + bars_header_metrics.1.line_height;

    // Static vertical components (excluding plot heights)
    let static_vertical = title_block_height
        + highlight_block_height
        + (spark_header_height + CARD_PADDING_Y)
        + (bars_header_height + CARD_PADDING_Y)
        + CANVAS_GUTTER * 3.0;

    fit_plots(
        &mut spark_plot_height,
        &mut bars_plot_height,
        MIN_SPARK_PLOT_HEIGHT,
        MIN_BARS_PLOT_HEIGHT,
        EXPORT_CANVAS_HEIGHT - CANVAS_MARGIN * 2.0,
        static_vertical,
    );

    let spark_card_height = spark_header_height + spark_plot_height + CARD_PADDING_Y;
    let bars_card_height = bars_header_height + bars_plot_height + CARD_PADDING_Y;

    let mut layout = Layout::new(EXPORT_CANVAS_WIDTH, CANVAS_MARGIN, CANVAS_GUTTER);
    let title_rect = layout.place_block(title_block_height);
    let highlight_rect = layout.place_block(highlight_block_height);
    let spark_rect = layout.place_block(spark_card_height);
    let bars_rect = layout.place_block(bars_card_height);

    // Inner rects with padding (avoid repeating math)
    let spark_inner = spark_rect.inset(CARD_PADDING_X, CARD_PADDING_Y / 2.0);
    let bars_inner = bars_rect.inset(CARD_PADDING_X, CARD_PADDING_Y / 2.0);

    // Layout report assert (debug builds)
    let report = layout.report(EXPORT_CANVAS_HEIGHT);
    debug_assert!(
        report.used_height <= report.canvas_height + 0.5,
        "export overflow: used {:.1} > canvas {:.1}",
        report.used_height,
        report.canvas_height
    );

    let total_meta = if overview.clean_runs > 0 {
        format!("{} clean", overview.clean_runs)
    } else if overview.total_runs == 0 {
        "Waiting on your first session".to_string()
    } else {
        "QC pending".to_string()
    };

    let pvt_value = format::format_ms(overview.avg_pvt.unwrap_or(f64::NAN));
    let pvt_meta = if overview.clean_pvt > 0 {
        format!("{} clean PVT sessions", overview.clean_pvt)
    } else {
        "Run a PVT to populate".to_string()
    };
    let accuracy_value = format::format_percent(overview.avg_nback_accuracy.unwrap_or(f64::NAN));
    let accuracy_meta = if overview.clean_nback > 0 {
        format!("{} clean 2-back sessions", overview.clean_nback)
    } else {
        "Complete a 2-back session".to_string()
    };
    let dprime_value = overview
        .avg_nback_dprime
        .filter(|value| value.is_finite())
        .map(|value| format!("{value:.1}"))
        .unwrap_or_else(|| "—".to_string());
    let dprime_meta = if overview.clean_nback > 0 {
        "Signal detection across sessions".to_string()
    } else {
        "Data pending".to_string()
    };

    let highlight_cards = [
        ("Total runs", overview.total_runs.to_string(), total_meta),
        ("Median PVT", pvt_value, pvt_meta),
        ("2-back accuracy", accuracy_value, accuracy_meta),
        ("2-back d′", dprime_value, dprime_meta),
    ];

    let mut svg = String::with_capacity(48_000);
    // scale supplied by caller (1.0 normal, 2.0 for @2x, etc.)

    // Snapshot-scoped IDs
    let mut ids = IdGen::new("llsnap");
    let clip_spark = ids.next("spark");
    let clip_bars = ids.next("bars");

    // We already have inner rects; derive plot widths/heights before defs so clipPaths can live in <defs>
    let spark_plot_width = spark_inner.width;
    let bars_plot_width = bars_inner.width;

    let inter_b64 = &*INTER_B64;

    let pixel_w = (EXPORT_CANVAS_WIDTH * scale).round();
    let pixel_h = (EXPORT_CANVAS_HEIGHT * scale).round();
    let _ = writeln!(
        svg,
        "<svg xmlns='http://www.w3.org/2000/svg' width='{:.0}' height='{:.0}' viewBox='0 0 {:.0} {:.0}' role='img'>",
        pixel_w,
        pixel_h,
        EXPORT_CANVAS_WIDTH,
        EXPORT_CANVAS_HEIGHT
    );
    let _ = writeln!(svg, "  <title>Looplace results snapshot</title>");
    let _ = writeln!(svg, "  <desc>Summary of recent cognitive runs</desc>");
    let _ = writeln!(svg, "  <style>@font-face{{font-family:'Inter Export';src:url(data:font/ttf;base64,{}) format('truetype');font-weight:100 900;font-style:normal;font-display:swap}}text{{font-family:'Inter Export',Inter,'Segoe UI',sans-serif}}</style>", inter_b64);
    let _ = writeln!(svg, "  <defs>");
    let _ = writeln!(
        svg,
        "    <linearGradient id='bg' x1='0' y1='0' x2='1' y2='1'>"
    );
    let _ = writeln!(svg, "      <stop offset='0%' stop-color='#151923'/>");
    let _ = writeln!(svg, "      <stop offset='100%' stop-color='#0f1116'/>");
    let _ = writeln!(svg, "    </linearGradient>");
    let _ = writeln!(
        svg,
        "    <clipPath id='{clip_spark}' clipPathUnits='userSpaceOnUse'><rect x='0' y='0' width='{:.2}' height='{:.2}' rx='8'/></clipPath>",
        spark_plot_width, spark_plot_height
    );
    let _ = writeln!(
        svg,
        "    <clipPath id='{clip_bars}' clipPathUnits='userSpaceOnUse'><rect x='0' y='0' width='{:.2}' height='{:.2}' rx='8'/></clipPath>",
        bars_plot_width, bars_plot_height
    );
    let _ = writeln!(svg, "  </defs>");
    let _ = writeln!(
        svg,
        "  <rect width='{:.0}' height='{:.0}' fill='{}'/>",
        EXPORT_CANVAS_WIDTH, EXPORT_CANVAS_HEIGHT, theme.background
    );

    // Title block
    let title_baseline = title_metrics.baseline;
    let subtitle_baseline = title_baseline + subtitle_metrics.line_height + 8.0;
    let meta_baseline = latest_line
        .as_ref()
        .map(|_| subtitle_baseline + subtitle_metrics.line_height + 6.0);

    let _ = writeln!(
        svg,
        "  <g transform='translate({:.2} {:.2})'>",
        title_rect.x, title_rect.y
    );
    let _ = writeln!(
        svg,
        "    <text x='0' y='{:.2}' fill='{}' font-family='{FONT_STACK}' font-size='56' font-weight='700'>Looplace results</text>",
        title_baseline,
        theme.text_primary
    );
    let _ = writeln!(
        svg,
        "    <text x='0' y='{:.2}' fill='{}' fill-opacity='0.72' font-family='{FONT_STACK}' font-size='26'>{}</text>",
        subtitle_baseline,
        theme.text_base,
        escape_text(&subtitle)
    );
    if let Some(meta) = latest_line.as_ref() {
        let _ = writeln!(
            svg,
            "    <text x='0' y='{:.2}' fill='{}' fill-opacity='{:.2}' font-family='{FONT_STACK}' font-size='18'>{}</text>",
            meta_baseline.unwrap(),
            theme.text_base,
            OP_TEXT_META,
            escape_text(meta)
        );
    }
    let _ = writeln!(svg, "  </g>");

    // Highlight cards
    let card_count = highlight_cards.len() as f64;
    let total_gap = HIGHLIGHT_GAP * (card_count - 1.0);
    let card_width = ((highlight_rect.width - total_gap) / card_count).floor();
    let highlight_label_baseline = CARD_PADDING_Y / 2.0 + highlight_label_metrics.baseline;
    let highlight_value_baseline =
        highlight_label_baseline + highlight_label_metrics.line_height + 6.0;
    let highlight_meta_baseline =
        highlight_value_baseline + highlight_value_metrics.line_height + 8.0;

    for (index, (label, value, meta)) in highlight_cards.iter().enumerate() {
        let card_x = crisp(highlight_rect.x + index as f64 * (card_width + HIGHLIGHT_GAP));
        let label_upper = label.to_ascii_uppercase();
        let _ = writeln!(
            svg,
            "  <g transform='translate({:.2} {:.2})'>",
            card_x, highlight_rect.y
        );
        let _ = writeln!(
            svg,
            "    <rect width='{:.2}' height='{:.2}' rx='{:.1}' fill='{}' fill-opacity='{:.3}' stroke='{}' stroke-opacity='{:.3}'/>",
            card_width, highlight_rect.height, CARD_RADIUS,
            theme.card_fill, OP_CARD_FILL, theme.card_stroke, OP_CARD_STROKE
        );
        let _ = writeln!(
            svg,
            "    <text x='{:.2}' y='{:.2}' fill='{}' fill-opacity='0.66' font-family='{FONT_STACK}' font-size='16' letter-spacing='0.08em'>{}</text>",
            CARD_PADDING_X,
            highlight_label_baseline,
            theme.text_base,
            escape_text(&label_upper)
        );
        let _ = writeln!(
            svg,
            "    <text x='{:.2}' y='{:.2}' fill='{}' font-family='{FONT_STACK}' font-size='36' font-weight='600'>{}</text>",
            CARD_PADDING_X,
            highlight_value_baseline,
            theme.text_primary,
            escape_text(value)
        );
        let _ = writeln!(
            svg,
            "    <text x='{:.2}' y='{:.2}' fill='{}' fill-opacity='{:.2}' font-family='{FONT_STACK}' font-size='15'>{}</text>",
            CARD_PADDING_X,
            highlight_meta_baseline,
            theme.text_base,
            OP_TEXT_META,
            escape_text(meta)
        );
        let _ = writeln!(svg, "  </g>");
    }

    // Spark card
    let spark_plot_width = spark_inner.width;
    let spark_plot_origin_y = spark_inner.y - spark_rect.y + spark_header_height;
    let spark_title_baseline = (CARD_PADDING_Y / 2.0) + spark_header_metrics.0.baseline;
    let spark_subtitle_baseline = spark_title_baseline + spark_header_metrics.0.line_height + 6.0;
    let spark_chart =
        build_sparkline_chart(&overview.spark_points, spark_plot_width, spark_plot_height);

    let _ = writeln!(
        svg,
        "  <g transform='translate({:.2} {:.2})'>",
        spark_rect.x, spark_rect.y
    );
    let _ = writeln!(
        svg,
        "    <rect width='{:.2}' height='{:.2}' rx='{:.1}' fill='{}' fill-opacity='{:.2}' stroke='{}' stroke-opacity='{:.2}'/>",
        spark_rect.width, spark_rect.height, CARD_RADIUS,
        theme.card_fill, OP_CARD_FILL, theme.card_stroke, OP_CARD_STROKE
    );
    let _ = writeln!(
        svg,
        "    <text x='{:.2}' y='{:.2}' fill='{}' font-family='{FONT_STACK}' font-size='24' font-weight='600'>PVT trend</text>",
        CARD_PADDING_X,
        spark_title_baseline,
        theme.text_primary
    );
    let _ = writeln!(
        svg,
        "    <text x='{:.2}' y='{:.2}' fill='{}' fill-opacity='{:.2}' font-family='{FONT_STACK}' font-size='16'>Median reaction time across clean runs</text>",
        CARD_PADDING_X,
        spark_subtitle_baseline,
        theme.text_base,
        OP_TEXT_MUTED
    );

    if let Some(chart) = spark_chart {
        let baseline_y = crisp(chart.height);
        let _ = writeln!(
            svg,
            "    <g transform='translate({:.2} {:.2})' clip-path='url(#{})' aria-roledescription='chart' shape-rendering='crispEdges' vector-effect='non-scaling-stroke'>",
            CARD_PADDING_X, (CARD_PADDING_Y / 2.0) + spark_header_height, clip_spark
        );
        let _ = writeln!(
            svg,
            "      <path d='{}' fill='{}' fill-opacity='{:.2}'/>",
            chart.fill_path, theme.accent, OP_ACCENT_SOFT
        );
        let _ = writeln!(
            svg,
            "      <path d='{}' fill='none' stroke='{}' stroke-width='3.2' stroke-linecap='round' stroke-linejoin='round'/>",
            chart.path,
            theme.accent
        );
        for (x, y) in &chart.markers {
            let _ = writeln!(
                svg,
                "      <circle cx='{:.2}' cy='{:.2}' r='3.4' fill='{}'/>",
                x, y, theme.accent
            );
        }
        let _ = writeln!(
            svg,
            "      <line x1='0' y1='{:.2}' x2='{:.2}' y2='{:.2}' stroke='{}' stroke-opacity='{:.2}' stroke-width='1'/>", // grid baseline
            baseline_y,
            chart.width,
            baseline_y,
            theme.card_stroke,
            OP_GRID
        );
        let _ = writeln!(
            svg,
            "      <text x='0' y='-14' fill='{}' fill-opacity='{:.2}' font-family='{FONT_STACK}' font-size='13'>{}</text>",
            theme.text_base,
            OP_TEXT_MUTED,
            escape_text(&chart.min_label)
        );
        let _ = writeln!(
            svg,
            "      <text x='{:.2}' y='-14' text-anchor='end' fill='{}' fill-opacity='{:.2}' font-family='{FONT_STACK}' font-size='13'>{}</text>",
            chart.width,
            theme.text_base,
            OP_TEXT_MUTED,
            escape_text(&chart.max_label)
        );
        if let Some(label) = &chart.start_label {
            let _ = writeln!(
                svg,
                "      <text x='0' y='{:.2}' fill='{}' fill-opacity='{:.2}' font-family='{FONT_STACK}' font-size='12'>{}</text>",
                chart.height + 24.0,
                theme.text_base,
                OP_TEXT_META,
                escape_text(label)
            );
        }
        if let Some(label) = &chart.end_label {
            let _ = writeln!(
                svg,
                "      <text x='{:.2}' y='{:.2}' text-anchor='end' fill='{}' fill-opacity='{:.2}' font-family='{FONT_STACK}' font-size='12'>{}</text>",
                chart.width,
                chart.height + 24.0,
                theme.text_base,
                OP_TEXT_META,
                escape_text(label)
            );
        }
        let _ = writeln!(svg, "    </g>");
    } else {
        let _ = writeln!(
            svg,
            "    <text x='{:.2}' y='{:.2}' fill='{}' fill-opacity='{:.2}' font-family='{FONT_STACK}' font-size='16'>Need more clean PVT runs to plot a trend.</text>",
            CARD_PADDING_X,
            spark_plot_origin_y + spark_plot_height / 2.0,
            theme.text_base,
            OP_TEXT_META
        );
    }
    let _ = writeln!(svg, "  </g>");

    // Bars card
    let bars_plot_width = bars_inner.width;
    let bars_plot_origin_y = bars_inner.y - bars_rect.y + bars_header_height;
    let bars_title_baseline = (CARD_PADDING_Y / 2.0) + bars_header_metrics.0.baseline;
    let bars_subtitle_baseline = bars_title_baseline + bars_header_metrics.0.line_height + 6.0;
    let bars_chart = build_bar_chart(
        theme.accent,
        &overview.bar_samples,
        bars_plot_width,
        bars_plot_height,
    );

    let _ = writeln!(
        svg,
        "  <g transform='translate({:.2} {:.2})'>",
        bars_rect.x, bars_rect.y
    );
    let _ = writeln!(
        svg,
        "    <rect width='{:.2}' height='{:.2}' rx='{:.1}' fill='{}' fill-opacity='{:.2}' stroke='{}' stroke-opacity='{:.2}'/>",
        bars_rect.width, bars_rect.height, CARD_RADIUS,
        theme.card_fill, OP_CARD_FILL, theme.card_stroke, OP_CARD_STROKE
    );
    let _ = writeln!(
        svg,
        "    <text x='{:.2}' y='{:.2}' fill='{}' font-family='{FONT_STACK}' font-size='24' font-weight='600'>Lapses vs false starts</text>",
        CARD_PADDING_X,
        bars_title_baseline,
        theme.text_primary
    );
    let _ = writeln!(
        svg,
        "    <text x='{:.2}' y='{:.2}' fill='{}' fill-opacity='{:.2}' font-family='{FONT_STACK}' font-size='16'>Recent clean PVT sessions</text>",
        CARD_PADDING_X,
        bars_subtitle_baseline,
        theme.text_base,
        OP_TEXT_MUTED
    );

    let legend_x = bars_rect.width - CARD_PADDING_X - 220.0;
    let legend_y = CARD_PADDING_Y / 2.0 - 6.0;
    let _ = writeln!(
        svg,
        "    <g transform='translate({:.2} {:.2})'>",
        legend_x, legend_y
    );
    let _ = writeln!(
        svg,
        "      <rect x='0' y='0' width='14' height='14' rx='4' fill='{}'/>",
        theme.accent
    );
    let _ = writeln!(
        svg,
        "      <text x='22' y='11' fill='{}' fill-opacity='{:.2}' font-family='{FONT_STACK}' font-size='14'>Lapses ≥500 ms</text>",
        theme.text_base,
        OP_TEXT_META
    );
    let _ = writeln!(
        svg,
        "      <rect x='132' y='0' width='14' height='14' rx='4' fill='{}' fill-opacity='0.32'/>",
        theme.accent
    );
    let _ = writeln!(
        svg,
        "      <text x='156' y='11' fill='{}' fill-opacity='{:.2}' font-family='{FONT_STACK}' font-size='14'>False starts</text>",
        theme.text_base,
        OP_TEXT_META
    );
    let _ = writeln!(svg, "    </g>");

    if let Some(chart) = bars_chart {
        let baseline_y = crisp(bars_plot_height);
        let _ = writeln!(
            svg,
            "    <g transform='translate({:.2} {:.2})' clip-path='url(#{})' aria-roledescription='chart' shape-rendering='crispEdges' vector-effect='non-scaling-stroke'>",
            CARD_PADDING_X, (CARD_PADDING_Y / 2.0) + bars_header_height, clip_bars
        );
        let _ = writeln!(
            svg,
            "      <line x1='0' y1='{:.2}' x2='{:.2}' y2='{:.2}' stroke='{}' stroke-opacity='{:.2}' stroke-width='1'/>", // grid baseline
            baseline_y,
            bars_plot_width,
            baseline_y,
            theme.card_stroke,
            OP_GRID
        );
        for bar in &chart.bars {
            let _ = writeln!(
                svg,
                "      <rect x='{:.2}' y='{:.2}' width='{:.2}' height='{:.2}' rx='4' fill='{}' fill-opacity='{:.2}'/>",
                                bar.x, bar.y, bar.width, bar.height, bar.color, bar.opacity
            );
        }
        for label in &chart.labels {
            let _ = writeln!(
                svg,
                "      <text x='{:.2}' y='{:.2}' text-anchor='middle' fill='{}' fill-opacity='{:.2}' font-family='{FONT_STACK}' font-size='12'>{}</text>",
                label.x,
                chart.height + 24.0,
                theme.text_base,
                OP_TEXT_META,
                escape_text(&label.text)
            );
        }
        let _ = writeln!(svg, "    </g>");
    } else {
        let _ = writeln!(
            svg,
            "    <text x='{:.2}' y='{:.2}' fill='{}' fill-opacity='{:.2}' font-family='{FONT_STACK}' font-size='16'>Complete clean PVT runs to compare lapses and false starts.</text>",
            CARD_PADDING_X,
            bars_plot_origin_y + bars_plot_height / 2.0,
            theme.text_base,
            OP_TEXT_META
        );
    }
    let _ = writeln!(svg, "  </g>");
    let _ = writeln!(svg, "</svg>");
    svg
}

#[cfg(not(target_arch = "wasm32"))]
fn svg_to_png(svg: &str, width: u32, height: u32) -> Result<Vec<u8>, String> {
    use png::{BitDepth, ColorType, Encoder};
    use resvg::render;
    use tiny_skia::{Pixmap, Transform};
    use usvg::{Options, Tree};

    let mut options = Options::default();
    {
        let db = options.fontdb_mut();
        db.load_system_fonts();
        #[cfg(feature = "embed_inter")]
        {
            db.load_font_data(include_bytes!("../../assets/Inter-Variable.ttf").to_vec());
            db.load_font_data(include_bytes!("../../assets/Inter-Italic-Variable.ttf").to_vec());
        }
    }

    let tree: Tree = Tree::from_data(svg.as_bytes(), &options)
        .map_err(|err| format!("SVG parse failed: {err:?}"))?;

    let mut pixmap = Pixmap::new(width, height).ok_or("Pixmap allocation failed")?;
    let mut pixmap_ref = pixmap.as_mut();
    render(&tree, Transform::default(), &mut pixmap_ref);

    let mut out = Vec::new();
    let mut encoder = Encoder::new(&mut out, width, height);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    encoder
        .write_header()
        .map_err(|err| err.to_string())?
        .write_image_data(pixmap.data())
        .map_err(|err| err.to_string())?;

    Ok(out)
}

struct SnapshotOverview {
    total_runs: usize,
    clean_runs: usize,
    clean_pvt: usize,
    clean_nback: usize,
    avg_pvt: Option<f64>,
    avg_nback_accuracy: Option<f64>,
    avg_nback_dprime: Option<f64>,
    latest_label: Option<String>,
    spark_points: Vec<SparkPoint>,
    bar_samples: Vec<BarSample>,
}

impl SnapshotOverview {
    fn build(records: &[SummaryRecord]) -> Self {
        let total_runs = records.len();
        let latest_label = records
            .iter()
            .max_by(|a, b| a.created_at.cmp(&b.created_at))
            .map(|record| format_timestamp(record));

        let mut clean_refs: Vec<&SummaryRecord> = records
            .iter()
            .filter(|record| record_is_clean(record))
            .collect();
        let clean_runs = clean_refs.len();
        if clean_refs.is_empty() {
            clean_refs = records.iter().collect();
        }

        let mut pvt_medians = Vec::new();
        let mut nback_accuracy = Vec::new();
        let mut nback_dprime = Vec::new();
        let mut clean_pvt = 0usize;
        let mut clean_nback = 0usize;

        let mut spark_collect: Vec<(OffsetDateTime, SparkPoint)> = Vec::new();
        let mut bar_collect: Vec<(OffsetDateTime, BarSample)> = Vec::new();

        for record in &clean_refs {
            if let Some(ts) = parse_timestamp(record) {
                match record.task.as_str() {
                    "pvt" => {
                        if let Some(metrics) = parse_pvt_metrics(record) {
                            if metrics.median_rt_ms.is_finite() {
                                pvt_medians.push(metrics.median_rt_ms);
                                spark_collect.push((
                                    ts,
                                    SparkPoint {
                                        label: format_date_badge(ts),
                                        value: metrics.median_rt_ms,
                                    },
                                ));
                                bar_collect.push((
                                    ts,
                                    BarSample {
                                        label: format_time_badge(ts),
                                        lapses: metrics.lapses_ge_500ms,
                                        false_starts: metrics.false_starts,
                                    },
                                ));
                                clean_pvt += 1;
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
                            clean_nback += 1;
                        }
                    }
                    _ => {}
                }
            }
        }

        spark_collect.sort_by(|a, b| a.0.cmp(&b.0));
        let spark_points: Vec<SparkPoint> =
            spark_collect.into_iter().map(|(_, point)| point).collect();

        bar_collect.sort_by(|a, b| a.0.cmp(&b.0));
        let mut bar_samples: Vec<BarSample> =
            bar_collect.into_iter().map(|(_, sample)| sample).collect();
        if bar_samples.len() > 8 {
            bar_samples = bar_samples.into_iter().rev().take(8).collect();
            bar_samples.reverse();
        }

        let avg_pvt = average_value(&pvt_medians);
        let avg_nback_accuracy = average_value(&nback_accuracy);
        let avg_nback_dprime = average_value(&nback_dprime);

        SnapshotOverview {
            total_runs,
            clean_runs,
            clean_pvt,
            clean_nback,
            avg_pvt,
            avg_nback_accuracy,
            avg_nback_dprime,
            latest_label,
            spark_points,
            bar_samples,
        }
    }
}

struct SparklineData {
    path: String,
    fill_path: String,
    markers: Vec<(f64, f64)>,
    min_label: String,
    max_label: String,
    start_label: Option<String>,
    end_label: Option<String>,
    width: f64,
    height: f64,
}

struct BarChartData {
    bars: Vec<BarRect>,
    labels: Vec<BarLabel>,
    height: f64,
}

struct BarRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    color: &'static str,
    opacity: f64,
}

struct BarLabel {
    x: f64,
    text: String,
}

struct SparkPoint {
    label: String,
    value: f64,
}

struct BarSample {
    label: String,
    lapses: u32,
    false_starts: u32,
}

fn build_sparkline_chart(points: &[SparkPoint], width: f64, height: f64) -> Option<SparklineData> {
    if points.len() < 2 {
        return None;
    }
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for point in points {
        if point.value.is_finite() {
            min = min.min(point.value);
            max = max.max(point.value);
        }
    }
    if !min.is_finite() || !max.is_finite() {
        return None;
    }
    let span = (max - min).max(1.0);
    let step = if points.len() > 1 {
        width / (points.len() - 1) as f64
    } else {
        width
    };
    let mut path = String::new();
    let mut fill_path = String::new();
    let mut markers = Vec::new();
    for (index, point) in points.iter().enumerate() {
        let x = step * index as f64;
        let norm = ((point.value - min) / span).clamp(0.0, 1.0);
        let y = height - norm * height;
        if index == 0 {
            let _ = write!(path, "M{:.2} {:.2}", x, y);
            let _ = write!(fill_path, "M{:.2} {:.2} L{:.2} {:.2}", x, height, x, y);
        } else {
            let _ = write!(path, " L{:.2} {:.2}", x, y);
            let _ = write!(fill_path, " L{:.2} {:.2}", x, y);
        }
        markers.push((x, y));
    }
    if let Some((last_x, _)) = markers.last() {
        let _ = write!(fill_path, " L{:.2} {:.2} Z", last_x, height);
    }
    let min_label = format!("MIN {} ms", min.round() as i64);
    let max_label = format!("MAX {} ms", max.round() as i64);
    let start_label = points.first().map(|p| p.label.clone());
    let end_label = points.last().map(|p| p.label.clone());
    Some(SparklineData {
        path,
        fill_path,
        markers,
        min_label,
        max_label,
        start_label,
        end_label,
        width,
        height,
    })
}

fn build_bar_chart(
    accent: &'static str,
    samples: &[BarSample],
    width: f64,
    height: f64,
) -> Option<BarChartData> {
    if samples.is_empty() {
        return None;
    }
    let max_value = samples
        .iter()
        .map(|sample| sample.lapses.max(sample.false_starts))
        .max()
        .unwrap_or(0)
        .max(1) as f64;

    let bar_width = 14.0;
    let pair_width = bar_width * 2.0 + 8.0;
    let margin = 20.0;
    let groups = samples.len() as f64;
    let available = (width - margin * 2.0) - pair_width * groups;
    let gap = if groups > 1.0 {
        (available.max(0.0)) / (groups - 1.0)
    } else {
        0.0
    };

    let mut bars = Vec::new();
    let mut labels = Vec::new();

    for (index, sample) in samples.iter().enumerate() {
        let group_x = margin + index as f64 * (pair_width + gap);
        let lapses_height = if sample.lapses == 0 {
            0.0
        } else {
            (sample.lapses as f64 / max_value) * height
        };
        let false_height = if sample.false_starts == 0 {
            0.0
        } else {
            (sample.false_starts as f64 / max_value) * height
        };
        let lapses_y = height - lapses_height;
        let false_y = height - false_height;
        bars.push(BarRect {
            x: crisp(group_x),
            y: crisp(lapses_y),
            width: bar_width,
            height: lapses_height,
            color: accent,
            opacity: 0.90,
        });
        bars.push(BarRect {
            x: crisp(group_x + bar_width + 8.0),
            y: crisp(false_y),
            width: bar_width,
            height: false_height,
            color: accent,
            opacity: 0.32,
        });
        labels.push(BarLabel {
            x: group_x + (pair_width / 2.0),
            text: ellipsize(&sample.label, 10),
        });
    }

    Some(BarChartData {
        bars,
        labels,
        height,
    })
}

fn average_value(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().copied().sum::<f64>() / values.len() as f64)
    }
}

fn escape_text(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

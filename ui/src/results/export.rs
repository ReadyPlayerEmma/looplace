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
const EXPORT_CANVAS_HEIGHT: f64 = 720.0;
const EXPORT_CONTENT_WIDTH: f64 = 960.0;
const SPARK_CHART_WIDTH: f64 = 880.0;
const SPARK_CHART_HEIGHT: f64 = 90.0;
const BAR_CHART_WIDTH: f64 = 880.0;
const BAR_CHART_HEIGHT: f64 = 70.0;

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
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(records.len() + 1);
    rows.push(
        [
            "task",
            "created_at",
            "platform",
            "tz",
            "median_rt_ms",
            "mean_rt_ms",
            "lapses_500ms",
            "false_starts",
            "slope_ms_per_min",
            "accuracy",
            "d_prime",
            "criterion",
            "hits",
            "misses",
            "false_alarms",
            "correct_rejections",
            "notes",
            "qc_summary",
            "qc_focus_lost",
            "qc_visibility_blur",
            "qc_min_trials_met",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
    );

    for record in records {
        let mut row = vec![
            record.task.as_str().to_string(),
            record.created_at.clone(),
            record.client.platform.clone(),
            record.client.tz.clone(),
        ];

        let mut pvt_fields = vec![String::new(); 5];
        let mut nback_fields = vec![String::new(); 6];

        match record.task.as_str() {
            "pvt" => {
                if let Some(metrics) = parse_pvt_metrics(record) {
                    pvt_fields = vec![
                        metrics.median_rt_ms.to_string(),
                        metrics.mean_rt_ms.to_string(),
                        metrics.lapses_ge_500ms.to_string(),
                        metrics.false_starts.to_string(),
                        metrics.time_on_task_slope_ms_per_min.to_string(),
                    ];
                }
            }
            "nback2" => {
                if let Some(metrics) = parse_nback_metrics(record) {
                    nback_fields = vec![
                        metrics.accuracy.to_string(),
                        metrics.d_prime.to_string(),
                        metrics.criterion.to_string(),
                        metrics.hits.to_string(),
                        metrics.misses.to_string(),
                        metrics.false_alarms.to_string(),
                        metrics.correct_rejections.to_string(),
                    ];
                }
            }
            _ => {}
        }

        if nback_fields.len() == 6 {
            nback_fields.insert(3, String::new());
        }

        let notes = record.notes.clone().unwrap_or_default();
        let qc = qc_summary(record);
        let qc_focus = record.qc.focus_lost_events.to_string();
        let qc_blur = record.qc.visibility_blur_events.to_string();
        let qc_min = record.qc.min_trials_met.to_string();

        row.extend(pvt_fields);
        row.extend(nback_fields);
        row.push(notes);
        row.push(qc);
        row.push(qc_focus);
        row.push(qc_blur);
        row.push(qc_min);

        rows.push(row);
    }

    let mut csv = String::new();
    for row in rows {
        let line = row
            .into_iter()
            .map(|field| escape_csv(&field))
            .collect::<Vec<_>>()
            .join(",");
        csv.push_str(&line);
        csv.push('\n');
    }

    csv
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
    let dir = dirs.data_dir().join("exports");
    Ok(dir)
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
    use web_sys::{
        Blob, BlobPropertyBag, CanvasRenderingContext2d, HtmlCanvasElement, HtmlImageElement, Url,
    };

    let svg_markup = svg_snapshot(records);
    let mut opts = BlobPropertyBag::new();
    opts.type_("image/svg+xml");
    let mut parts = js_sys::Array::new();
    parts.push(&JsValue::from_str(&svg_markup));
    let blob = Blob::new_with_str_sequence_and_options(&parts, &opts)
        .map_err(|_| "Unable to build SVG blob".to_string())?;
    let url = Url::create_object_url_with_blob(&blob)
        .map_err(|_| "Unable to create SVG URL".to_string())?;

    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or("Document unavailable")?;

    let canvas: HtmlCanvasElement = document
        .create_element("canvas")
        .map_err(|_| "Unable to create canvas")?
        .dyn_into()
        .map_err(|_| "Canvas cast failed")?;
    canvas.set_width(1200);
    canvas.set_height(720);

    let context: CanvasRenderingContext2d = canvas
        .get_context("2d")
        .map_err(|_| "Canvas context unavailable")?
        .ok_or("Canvas context missing")?
        .dyn_into()
        .map_err(|_| "Context cast failed")?;

    let image = HtmlImageElement::new().map_err(|_| "Unable to create image")?;
    let decode = image.decode();
    image.set_src(&url);
    JsFuture::from(decode)
        .await
        .map_err(|_| "Image decode failed")?;

    context
        .draw_image_with_html_image_element(&image, 0.0, 0.0)
        .map_err(|_| "Unable to draw image")?;

    let data_url = canvas
        .to_data_url_with_type("image/png")
        .map_err(|_| "Unable to serialise canvas")?;
    Url::revoke_object_url(&url).ok();

    let bytes = base64::decode(data_url.split(',').nth(1).ok_or("Malformed data URL")?)
        .map_err(|_| "PNG decode failed")?;

    Ok(bytes)
}

#[cfg(not(target_arch = "wasm32"))]
fn build_png_desktop(records: &[SummaryRecord]) -> Result<Vec<u8>, String> {
    let svg_markup = svg_snapshot(records);
    svg_to_png(&svg_markup, 1200, 720)
}

fn svg_snapshot(records: &[SummaryRecord]) -> String {
    let overview = SnapshotOverview::build(records);

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

    let highlight_width = 230.0;
    let highlight_height = 112.0;
    let highlight_gap = 20.0;
    let highlight_y = 220.0;
    let content_margin = 80.0;

    let spark_y = 350.0;
    let spark_bg_height = 220.0;
    let spark_chart_offset_x = 32.0;
    let spark_chart_offset_y = 118.0;

    let bar_y = 560.0;
    let bar_bg_height = 160.0;
    let bar_chart_offset_x = 32.0;
    let bar_chart_offset_y = 88.0;

    let mut svg = String::new();

    let _ = writeln!(
        svg,
        "<svg xmlns='http://www.w3.org/2000/svg' width='{:.0}' height='{:.0}' viewBox='0 0 {:.0} {:.0}' role='img'>",
        EXPORT_CANVAS_WIDTH,
        EXPORT_CANVAS_HEIGHT,
        EXPORT_CANVAS_WIDTH,
        EXPORT_CANVAS_HEIGHT
    );
    let _ = writeln!(svg, "  <defs>");
    let _ = writeln!(
        svg,
        "    <linearGradient id='bg' x1='0' y1='0' x2='1' y2='1'>"
    );
    let _ = writeln!(svg, "      <stop offset='0%' stop-color='#151923'/>");
    let _ = writeln!(svg, "      <stop offset='100%' stop-color='#0f1116'/>");
    let _ = writeln!(svg, "    </linearGradient>");
    let _ = writeln!(svg, "  </defs>");
    let _ = writeln!(
        svg,
        "  <rect width='{:.0}' height='{:.0}' fill='url(#bg)'/>",
        EXPORT_CANVAS_WIDTH, EXPORT_CANVAS_HEIGHT
    );

    let _ = writeln!(
        svg,
        "  <text x='64' y='136' fill='#f5f7fb' font-family='Inter, Segoe UI, sans-serif' font-size='56' font-weight='700'>Looplace results</text>"
    );
    let _ = writeln!(
        svg,
        "  <text x='64' y='186' fill='rgba(245,247,251,0.72)' font-family='Inter, Segoe UI, sans-serif' font-size='26'>{}</text>",
        escape_text(&subtitle)
    );

    if let Some(latest) = latest_line {
        let _ = writeln!(
            svg,
            "  <text x='64' y='214' fill='rgba(245,247,251,0.55)' font-family='Inter, Segoe UI, sans-serif' font-size='18'>{}</text>",
            escape_text(&latest)
        );
    }

    for (index, (label, value, meta)) in highlight_cards.iter().enumerate() {
        let offset_x = content_margin + index as f64 * (highlight_width + highlight_gap);
        let label_upper = label.to_ascii_uppercase();
        let _ = writeln!(
            svg,
            "  <g transform='translate({offset_x:.0} {highlight_y:.0})'>",
        );
        let _ = writeln!(
            svg,
            "    <rect width='{highlight_width:.0}' height='{highlight_height:.0}' rx='20' fill='rgba(255,255,255,0.05)' stroke='rgba(255,255,255,0.08)'/>"
        );
        let _ = writeln!(
            svg,
            "    <text x='20' y='38' fill='rgba(245,247,251,0.66)' font-family='Inter, Segoe UI, sans-serif' font-size='16' letter-spacing='0.08em'>{}</text>",
            escape_text(&label_upper)
        );
        let _ = writeln!(
            svg,
            "    <text x='20' y='74' fill='#f5f7fb' font-family='Inter, Segoe UI, sans-serif' font-size='36' font-weight='600'>{}</text>",
            escape_text(value)
        );
        let _ = writeln!(
            svg,
            "    <text x='20' y='98' fill='rgba(245,247,251,0.55)' font-family='Inter, Segoe UI, sans-serif' font-size='15'>{}</text>",
            escape_text(meta)
        );
        let _ = writeln!(svg, "  </g>");
    }

    let _ = writeln!(
        svg,
        "  <g transform='translate({content_margin:.0} {spark_y:.0})'>"
    );
    let _ = writeln!(
        svg,
        "    <rect width='{EXPORT_CONTENT_WIDTH:.0}' height='{spark_bg_height:.0}' rx='24' fill='rgba(255,255,255,0.05)' stroke='rgba(255,255,255,0.08)'/>"
    );
    let _ = writeln!(
        svg,
        "    <text x='32' y='48' fill='rgba(245,247,251,0.9)' font-family='Inter, Segoe UI, sans-serif' font-size='24' font-weight='600'>PVT trend</text>"
    );
    let _ = writeln!(
        svg,
        "    <text x='32' y='76' fill='rgba(245,247,251,0.62)' font-family='Inter, Segoe UI, sans-serif' font-size='16'>Median reaction time across clean runs</text>"
    );

    if let Some(spark) = &overview.spark {
        let _ = writeln!(
            svg,
            "    <g transform='translate({spark_chart_offset_x:.0} {spark_chart_offset_y:.0})'>"
        );
        let _ = writeln!(
            svg,
            "      <path d='{}' fill='rgba(240,90,126,0.12)'/>",
            spark.fill_path
        );
        let _ = writeln!(
            svg,
            "      <path d='{}' fill='none' stroke='#f05a7e' stroke-width='3.2' stroke-linecap='round' stroke-linejoin='round'/>",
            spark.path
        );
        for (x, y) in &spark.markers {
            let _ = writeln!(
                svg,
                "      <circle cx='{x:.2}' cy='{y:.2}' r='3.4' fill='#f05a7e'/>"
            );
        }
        let _ = writeln!(
            svg,
            "      <line x1='0' y1='{SPARK_CHART_HEIGHT:.2}' x2='{SPARK_CHART_WIDTH:.2}' y2='{SPARK_CHART_HEIGHT:.2}' stroke='rgba(255,255,255,0.08)' stroke-width='1.4'/>"
        );
        let _ = writeln!(
            svg,
            "      <text x='0' y='-18' fill='rgba(245,247,251,0.62)' font-family='Inter, Segoe UI, sans-serif' font-size='13'>{}</text>",
            escape_text(&spark.min_label)
        );
        let _ = writeln!(
            svg,
            "      <text x='{SPARK_CHART_WIDTH:.2}' y='-18' text-anchor='end' fill='rgba(245,247,251,0.62)' font-family='Inter, Segoe UI, sans-serif' font-size='13'>{}</text>",
            escape_text(&spark.max_label)
        );
        if let Some(label) = &spark.start_label {
            let _ = writeln!(
                svg,
                "      <text x='0' y='{:.2}' fill='rgba(245,247,251,0.55)' font-family='Inter, Segoe UI, sans-serif' font-size='12'>{}</text>",
                SPARK_CHART_HEIGHT + 24.0,
                escape_text(label)
            );
        }
        if let Some(label) = &spark.end_label {
            let _ = writeln!(
                svg,
                "      <text x='{SPARK_CHART_WIDTH:.2}' y='{:.2}' text-anchor='end' fill='rgba(245,247,251,0.55)' font-family='Inter, Segoe UI, sans-serif' font-size='12'>{}</text>",
                SPARK_CHART_HEIGHT + 24.0,
                escape_text(label)
            );
        }
        let _ = writeln!(svg, "    </g>");
    } else {
        let _ = writeln!(
            svg,
            "    <text x='32' y='{:.0}' fill='rgba(245,247,251,0.55)' font-family='Inter, Segoe UI, sans-serif' font-size='16'>Need more clean PVT runs to plot a trend.</text>",
            spark_bg_height / 2.0 + 10.0
        );
    }
    let _ = writeln!(svg, "  </g>");

    let _ = writeln!(
        svg,
        "  <g transform='translate({content_margin:.0} {bar_y:.0})'>"
    );
    let _ = writeln!(
        svg,
        "    <rect width='{EXPORT_CONTENT_WIDTH:.0}' height='{bar_bg_height:.0}' rx='24' fill='rgba(255,255,255,0.05)' stroke='rgba(255,255,255,0.08)'/>"
    );
    let _ = writeln!(
        svg,
        "    <text x='32' y='44' fill='rgba(245,247,251,0.9)' font-family='Inter, Segoe UI, sans-serif' font-size='24' font-weight='600'>Lapses vs false starts</text>"
    );
    let _ = writeln!(
        svg,
        "    <text x='32' y='70' fill='rgba(245,247,251,0.62)' font-family='Inter, Segoe UI, sans-serif' font-size='16'>Recent clean PVT sessions</text>"
    );

    let legend_origin = EXPORT_CONTENT_WIDTH - 240.0;
    let _ = writeln!(svg, "    <g transform='translate({legend_origin:.0} 36)'>");
    let _ = writeln!(
        svg,
        "      <rect x='0' y='0' width='14' height='14' rx='4' fill='rgba(240,90,126,0.9)'/>"
    );
    let _ = writeln!(
        svg,
        "      <text x='22' y='11' fill='rgba(245,247,251,0.78)' font-family='Inter, Segoe UI, sans-serif' font-size='14'>Lapses ≥500 ms</text>"
    );
    let _ = writeln!(
        svg,
        "      <rect x='132' y='0' width='14' height='14' rx='4' fill='rgba(240,90,126,0.32)'/>"
    );
    let _ = writeln!(
        svg,
        "      <text x='156' y='11' fill='rgba(245,247,251,0.78)' font-family='Inter, Segoe UI, sans-serif' font-size='14'>False starts</text>"
    );
    let _ = writeln!(svg, "    </g>");

    if let Some(chart) = &overview.bars {
        let _ = writeln!(
            svg,
            "    <g transform='translate({bar_chart_offset_x:.0} {bar_chart_offset_y:.0})'>"
        );
        let _ = writeln!(
            svg,
            "      <line x1='0' y1='{BAR_CHART_HEIGHT:.2}' x2='{BAR_CHART_WIDTH:.2}' y2='{BAR_CHART_HEIGHT:.2}' stroke='rgba(255,255,255,0.08)' stroke-width='1.4'/>"
        );
        for bar in &chart.bars {
            let _ = writeln!(
                svg,
                "      <rect x='{:.2}' y='{:.2}' width='{:.2}' height='{:.2}' rx='4' fill='{}'/>",
                bar.x, bar.y, bar.width, bar.height, bar.color
            );
        }
        for label in &chart.labels {
            let _ = writeln!(
                svg,
                "      <text x='{:.2}' y='{:.2}' text-anchor='middle' fill='rgba(245,247,251,0.55)' font-family='Inter, Segoe UI, sans-serif' font-size='12'>{}</text>",
                label.x,
                BAR_CHART_HEIGHT + 24.0,
                escape_text(&label.text)
            );
        }
        let _ = writeln!(svg, "    </g>");
    } else {
        let _ = writeln!(
            svg,
            "    <text x='32' y='{:.0}' fill='rgba(245,247,251,0.55)' font-family='Inter, Segoe UI, sans-serif' font-size='16'>Complete clean PVT runs to compare lapses and false starts.</text>",
            bar_bg_height / 2.0 + 10.0
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
    options.fontdb_mut().load_system_fonts();

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
    spark: Option<SparklineData>,
    bars: Option<BarChartData>,
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
        let spark = build_sparkline_chart(&spark_points);

        bar_collect.sort_by(|a, b| a.0.cmp(&b.0));
        let mut bar_samples: Vec<BarSample> =
            bar_collect.into_iter().map(|(_, sample)| sample).collect();
        if bar_samples.len() > 8 {
            bar_samples = bar_samples.into_iter().rev().take(8).collect();
            bar_samples.reverse();
        }
        let bars = build_bar_chart(&bar_samples);

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
            spark,
            bars,
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
}

struct BarChartData {
    bars: Vec<BarRect>,
    labels: Vec<BarLabel>,
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

struct SparkPoint {
    label: String,
    value: f64,
}

struct BarSample {
    label: String,
    lapses: u32,
    false_starts: u32,
}

fn build_sparkline_chart(points: &[SparkPoint]) -> Option<SparklineData> {
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
        SPARK_CHART_WIDTH / (points.len() - 1) as f64
    } else {
        SPARK_CHART_WIDTH
    };

    let mut path = String::new();
    let mut fill_path = String::new();
    let mut markers = Vec::new();

    for (index, point) in points.iter().enumerate() {
        let x = step * index as f64;
        let norm = ((point.value - min) / span).clamp(0.0, 1.0);
        let y = SPARK_CHART_HEIGHT - norm * SPARK_CHART_HEIGHT;

        if index == 0 {
            let _ = write!(path, "M{:.2} {:.2}", x, y);
            let _ = write!(
                fill_path,
                "M{:.2} {:.2} L{:.2} {:.2}",
                x, SPARK_CHART_HEIGHT, x, y
            );
        } else {
            let _ = write!(path, " L{:.2} {:.2}", x, y);
            let _ = write!(fill_path, " L{:.2} {:.2}", x, y);
        }

        markers.push((x, y));
    }

    if let Some((last_x, _)) = markers.last() {
        let _ = write!(fill_path, " L{:.2} {:.2} Z", last_x, SPARK_CHART_HEIGHT);
    }

    let min_label = format!("MIN {} ms", min.round() as i64);
    let max_label = format!("MAX {} ms", max.round() as i64);
    let start_label = points.first().map(|point| point.label.clone());
    let end_label = points.last().map(|point| point.label.clone());

    Some(SparklineData {
        path,
        fill_path,
        markers,
        min_label,
        max_label,
        start_label,
        end_label,
    })
}

fn build_bar_chart(samples: &[BarSample]) -> Option<BarChartData> {
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
    let available = (BAR_CHART_WIDTH - margin * 2.0) - pair_width * groups;
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
            (sample.lapses as f64 / max_value) * BAR_CHART_HEIGHT
        };
        let false_height = if sample.false_starts == 0 {
            0.0
        } else {
            (sample.false_starts as f64 / max_value) * BAR_CHART_HEIGHT
        };

        let lapses_y = BAR_CHART_HEIGHT - lapses_height;
        let false_y = BAR_CHART_HEIGHT - false_height;

        bars.push(BarRect {
            x: group_x,
            y: lapses_y,
            width: bar_width,
            height: lapses_height,
            color: "rgba(240,90,126,0.9)",
        });

        bars.push(BarRect {
            x: group_x + bar_width + 8.0,
            y: false_y,
            width: bar_width,
            height: false_height,
            color: "rgba(240,90,126,0.32)",
        });

        labels.push(BarLabel {
            x: group_x + (pair_width / 2.0),
            text: sample.label.clone(),
        });
    }

    Some(BarChartData { bars, labels })
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
}

use dioxus::prelude::*;

#[cfg(target_arch = "wasm32")]
use crate::core::platform;
use crate::core::storage::SummaryRecord;
use crate::results::{parse_nback_metrics, parse_pvt_metrics, qc_summary};

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
    let header = "Looplace results";
    let sub = format!("{} runs saved locally", records.len());
    format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='1200' height='720' viewBox='0 0 1200 720'>\n  <defs>\n    <linearGradient id='bg' x1='0' y1='0' x2='1' y2='1'>\n      <stop offset='0%' stop-color='#151923'/>\n      <stop offset='100%' stop-color='#0f1116'/>\n    </linearGradient>\n  </defs>\n  <rect width='1200' height='720' fill='url(#bg)'/>\n  <text x='60' y='140' fill='#f5f7fb' font-family='Inter, \"Segoe UI\", sans-serif' font-size='56' font-weight='700'>{header}</text>\n  <text x='60' y='190' fill='rgba(245,247,251,0.72)' font-family='Inter, \"Segoe UI\", sans-serif' font-size='28'>{sub}</text>\n</svg>"
    )
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

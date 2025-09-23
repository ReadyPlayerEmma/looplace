use crate::{
    core::storage::SummaryRecord,
    tasks::{nback::NBackMetrics, pvt::PvtMetrics},
};
use time::{format_description::well_known::Rfc3339, macros::format_description, OffsetDateTime};

pub(crate) fn format_timestamp(record: &SummaryRecord) -> String {
    let iso = record.created_at.as_str();
    let (date, time_segment) = iso.split_once('T').unwrap_or((iso, ""));

    let primary_time = time_segment
        .split(['.', 'Z', '+'])
        .next()
        .unwrap_or(time_segment);

    let time_display: String = primary_time.chars().take(5).collect();

    let mut label = if !time_display.is_empty() {
        format!("{date} · {time_display}")
    } else {
        date.to_string()
    };

    if !record.client.tz.is_empty() {
        label.push_str(" · ");
        label.push_str(record.client.tz.as_str());
    }

    label
}

pub(crate) fn parse_timestamp(record: &SummaryRecord) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(record.created_at.as_str(), &Rfc3339).ok()
}

pub(crate) fn format_date_badge(date: OffsetDateTime) -> String {
    date.format(&format_description!(
        "[month repr:short] [day padding:none]"
    ))
    .unwrap_or_else(|_| "—".to_string())
}

pub(crate) fn format_time_badge(date: OffsetDateTime) -> String {
    date.format(&format_description!("[hour]:[minute]"))
        .unwrap_or_else(|_| "—".to_string())
}

pub(crate) fn qc_summary(record: &SummaryRecord) -> String {
    let qc = &record.qc;
    let mut parts = Vec::new();

    if qc.focus_lost_events > 0 {
        parts.push(format!("Focus lost ×{}", qc.focus_lost_events));
    }
    if qc.visibility_blur_events > 0 {
        parts.push(format!("Window blur ×{}", qc.visibility_blur_events));
    }
    if !qc.min_trials_met {
        parts.push("Min trials not met".to_string());
    }

    if parts.is_empty() {
        "QC: clean run".to_string()
    } else {
        format!("QC: {}", parts.join(", "))
    }
}

pub(crate) fn record_is_clean(record: &SummaryRecord) -> bool {
    let qc = &record.qc;
    qc.min_trials_met && qc.focus_lost_events == 0 && qc.visibility_blur_events == 0
}

pub(crate) fn task_label(task: &str) -> &'static str {
    match task {
        "pvt" => "Psychomotor Vigilance",
        "nback2" => "2-back working memory",
        _ => "Session",
    }
}

pub(crate) fn format_device(platform: &str, tz: &str) -> Option<String> {
    let platform_trimmed = platform.trim();
    let tz_trimmed = tz.trim();

    match (platform_trimmed.is_empty(), tz_trimmed.is_empty()) {
        (true, true) => None,
        (false, true) => Some(platform_trimmed.to_string()),
        (true, false) => Some(tz_trimmed.to_string()),
        (false, false) => Some(format!("{platform_trimmed} · {tz_trimmed}")),
    }
}

pub(crate) fn parse_pvt_metrics(record: &SummaryRecord) -> Option<PvtMetrics> {
    serde_json::from_value(record.metrics.clone()).ok()
}

pub(crate) fn parse_nback_metrics(record: &SummaryRecord) -> Option<NBackMetrics> {
    serde_json::from_value(record.metrics.clone()).ok()
}

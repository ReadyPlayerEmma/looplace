//! Convert domain data (Libre readings, Looplace cognition summaries) into the
//! tidy [`Observation`] shape.

use std::collections::BTreeMap;

use serde::Deserialize;
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, PrimitiveDateTime};

use looplace_libre::records::{Annotations, GlucoseSource, Reading};

use crate::error::Result;
use crate::observation::Observation;

/// Convert a Libre [`Reading`] into an [`Observation`]. `source` is the device
/// serial (e.g. from `$sn?`). Time-adjustment events return `None` (metadata,
/// not a measurement).
pub fn reading_to_observation(reading: &Reading, source: &str) -> Option<Observation> {
    match reading {
        Reading::Glucose {
            timestamp,
            value_mg_dl,
            source: kind,
            device_id,
            annotations,
            ..
        } => {
            let mut obs = Observation::new("glucose.mg_dl", *timestamp, *value_mg_dl as f64, "mg/dL", source);
            obs.tags.insert("kind".into(), glucose_kind(*kind).into());
            obs.tags.insert("record_seq".into(), device_id.to_string());
            annotate(&mut obs.tags, annotations);
            Some(obs)
        }
        Reading::Ketone {
            timestamp,
            value_mmol_l,
            device_id,
            annotations,
            ..
        } => {
            let mut obs = Observation::new("ketone.mmol_l", *timestamp, *value_mmol_l, "mmol/L", source);
            obs.tags.insert("record_seq".into(), device_id.to_string());
            annotate(&mut obs.tags, annotations);
            Some(obs)
        }
        Reading::TimeAdjustment { .. } => None,
    }
}

fn glucose_kind(source: GlucoseSource) -> &'static str {
    match source {
        GlucoseSource::SensorHistory => "sensor",
        GlucoseSource::Scan => "scan",
        GlucoseSource::BloodSample => "blood",
    }
}

fn annotate(tags: &mut BTreeMap<String, String>, a: &Annotations) {
    if a.food {
        tags.insert("food".into(), "true".into());
    }
    if let Some(g) = a.food_carbs_grams {
        tags.insert("food_carbs_grams".into(), g.to_string());
    }
    if a.sport {
        tags.insert("exercise".into(), "true".into());
    }
    if a.medication {
        tags.insert("medication".into(), "true".into());
    }
    if let Some(u) = a.long_acting_insulin_units {
        tags.insert("long_acting_insulin_units".into(), format!("{u:.1}"));
    }
    if let Some(u) = a.rapid_acting_insulin_units {
        tags.insert("rapid_acting_insulin_units".into(), format!("{u:.1}"));
    }
}

/// A minimal mirror of Looplace's `SummaryRecord` for migration — only the
/// fields we need, so this crate need not depend on `looplace-ui`.
#[derive(Debug, Clone, Deserialize)]
pub struct CognitionSummary {
    pub id: String,
    pub task: String,
    pub created_at: String,
    pub metrics: serde_json::Value,
}

/// Parse the legacy `summaries.json` array.
pub fn summaries_from_json(json: &str) -> Result<Vec<CognitionSummary>> {
    Ok(serde_json::from_str(json)?)
}

/// Flatten one cognition summary into one observation per numeric metric.
pub fn summary_to_observations(summary: &CognitionSummary) -> Vec<Observation> {
    let Some(timestamp) = parse_rfc3339(&summary.created_at) else {
        return Vec::new();
    };
    let Some(metrics) = summary.metrics.as_object() else {
        return Vec::new();
    };

    metrics
        .iter()
        .filter_map(|(key, value)| {
            let num = value.as_f64()?;
            let mut obs = Observation::new(
                format!("{}.{}", summary.task, key),
                timestamp,
                num,
                unit_for_metric(key),
                "looplace",
            );
            obs.session_id = Some(summary.id.clone());
            Some(obs)
        })
        .collect()
}

/// Best-effort unit from a metric key suffix.
fn unit_for_metric(key: &str) -> &'static str {
    if key.ends_with("_ms") || key.ends_with("_ms_per_min") {
        "ms"
    } else if key.ends_with("_pct") || key.contains("accuracy") {
        "%"
    } else {
        ""
    }
}

/// Parse an RFC3339 instant into a naive local-equivalent timestamp.
///
/// NOTE: this currently keeps the UTC wall-clock. Aligning cognition (UTC) with
/// device-local glucose is the documented time-basis TODO.
fn parse_rfc3339(s: &str) -> Option<PrimitiveDateTime> {
    let odt = OffsetDateTime::parse(s, &Rfc3339).ok()?;
    let utc = odt.to_offset(time::UtcOffset::UTC);
    Some(PrimitiveDateTime::new(utc.date(), utc.time()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use looplace_libre::records::parse_arresult_record;

    fn raw(line: &str) -> Vec<String> {
        line.split(',').map(|s| s.to_string()).collect()
    }

    #[test]
    fn glucose_scan_with_annotations_to_observation() {
        // Real fw-1.5.11 record: 94 mg/dL scan with food 15 g + exercise.
        let reading = parse_arresult_record(&raw(
            "453,2,6,19,26,9,11,30,1,2,0,0,94,1,3,1,0,0,0,0,0,0,3,0,0,1,15,0,0,276,94,5,\"\",\"\",\"\",\"\",\"\",\"\"",
        ))
        .unwrap();
        let obs = reading_to_observation(&reading, "MPGF176-T4167").unwrap();
        assert_eq!(obs.stream, "glucose.mg_dl");
        assert_eq!(obs.value, 94.0);
        assert_eq!(obs.unit, "mg/dL");
        assert_eq!(obs.source, "MPGF176-T4167");
        assert_eq!(obs.tags.get("kind").map(String::as_str), Some("scan"));
        assert_eq!(obs.tags.get("food_carbs_grams").map(String::as_str), Some("15"));
        assert_eq!(obs.tags.get("exercise").map(String::as_str), Some("true"));
    }

    #[test]
    fn cognition_summary_flattens_numeric_metrics() {
        let json = r#"[
            {
              "id": "pvt-1",
              "task": "pvt",
              "created_at": "2026-06-19T08:00:00Z",
              "metrics": { "median_rt_ms": 312.5, "lapses_ge_500ms": 2, "notes": "x" }
            }
        ]"#;
        let summaries = summaries_from_json(json).unwrap();
        let obs = summary_to_observations(&summaries[0]);
        // Two numeric metrics; the string "notes" is skipped.
        assert_eq!(obs.len(), 2);
        let median = obs.iter().find(|o| o.stream == "pvt.median_rt_ms").unwrap();
        assert_eq!(median.value, 312.5);
        assert_eq!(median.unit, "ms");
        assert_eq!(median.session_id.as_deref(), Some("pvt-1"));
    }
}

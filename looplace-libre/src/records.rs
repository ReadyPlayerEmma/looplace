//! Record parsing and the reading model for FreeStyle Libre devices.
//!
//! Ported from glucometerutils' `support/freestyle_libre.py` (MIT). Records arrive
//! as comma-separated string fields (rows from [`crate::session::Session::query_multirecord`]);
//! `$history?` yields the CGM sensor trace and `$arresult?` yields scans, blood/
//! ketone tests, annotations, and clock adjustments.
//!
//! Glucose values are the device-internal **mg/dL**; ketone values are converted
//! to **mmol/L** (raw / 18), matching the reference driver.

use time::{Date, Month, PrimitiveDateTime, Time};

/// Where a glucose value originated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlucoseSource {
    /// Background CGM sensor trace (`$history?`).
    SensorHistory,
    /// An explicit user scan of the sensor.
    Scan,
    /// A blood-glucose strip test.
    BloodSample,
}

/// On-device annotations attached to an `$arresult?` event.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Annotations {
    pub sport: bool,
    pub medication: bool,
    pub food: bool,
    /// Carbohydrate grams, when the food flag is set and grams were recorded.
    pub food_carbs_grams: Option<u32>,
    pub long_acting_insulin: bool,
    /// Long-acting insulin units (device stores 2× units; halved here).
    pub long_acting_insulin_units: Option<f64>,
    pub rapid_acting_insulin: bool,
    /// Rapid-acting insulin units (present only when the device recorded it).
    pub rapid_acting_insulin_units: Option<f64>,
    /// Selected user "custom comment" strings.
    pub custom_comments: Vec<String>,
}

/// A parsed reading from the device.
#[derive(Debug, Clone, PartialEq)]
pub enum Reading {
    /// A glucose value in mg/dL.
    Glucose {
        timestamp: PrimitiveDateTime,
        value_mg_dl: i64,
        source: GlucoseSource,
        device_id: i64,
        annotations: Annotations,
        /// Human-readable comment, identical to the reference driver's.
        comment: String,
    },
    /// A ketone value in mmol/L.
    Ketone {
        timestamp: PrimitiveDateTime,
        value_mmol_l: f64,
        device_id: i64,
        annotations: Annotations,
        comment: String,
    },
    /// The device clock was changed (`new` is the corrected time).
    TimeAdjustment {
        timestamp: PrimitiveDateTime,
        old_timestamp: PrimitiveDateTime,
        device_id: i64,
    },
}

/// Parse the integer field at `idx` (trimmed), as `int()` does in the reference.
fn field(record: &[String], idx: usize) -> Option<i64> {
    record.get(idx)?.trim().parse::<i64>().ok()
}

/// Build a timestamp from the device's components (2-digit year + 2000).
fn extract_timestamp(
    year2: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
) -> Option<PrimitiveDateTime> {
    let year = i32::try_from(year2).ok()? + 2000;
    let month = Month::try_from(u8::try_from(month).ok()?).ok()?;
    let date = Date::from_calendar_date(year, month, u8::try_from(day).ok()?).ok()?;
    let time = Time::from_hms(
        u8::try_from(hour).ok()?,
        u8::try_from(minute).ok()?,
        u8::try_from(second).ok()?,
    )
    .ok()?;
    Some(PrimitiveDateTime::new(date, time))
}

/// Timestamp from the base record fields (idx 2..=7: month, day, year, h, m, s).
fn base_timestamp(record: &[String]) -> Option<PrimitiveDateTime> {
    extract_timestamp(
        field(record, 4)?,
        field(record, 2)?,
        field(record, 3)?,
        field(record, 5)?,
        field(record, 6)?,
        field(record, 7)?,
    )
}

/// Parse one `$history?` record (a background CGM sensor reading).
pub fn parse_history_record(record: &[String]) -> Option<Reading> {
    let device_id = field(record, 0)?;
    let timestamp = base_timestamp(record)?;
    let value_mg_dl = field(record, 13)?;
    let errors = field(record, 15)?;
    if errors != 0 {
        return None;
    }
    Some(Reading::Glucose {
        timestamp,
        value_mg_dl,
        source: GlucoseSource::SensorHistory,
        device_id,
        annotations: Annotations::default(),
        comment: "(Sensor)".into(),
    })
}

/// Parse one `$arresult?` record (scan / blood / ketone / clock adjustment).
pub fn parse_arresult_record(record: &[String]) -> Option<Reading> {
    let device_id = field(record, 0)?;
    let record_type = field(record, 1)?;
    let timestamp = base_timestamp(record)?;

    match record_type {
        2 => parse_type2(record, device_id, timestamp),
        5 => {
            let old_timestamp = extract_timestamp(
                field(record, 11)?,
                field(record, 9)?,
                field(record, 10)?,
                field(record, 12)?,
                field(record, 13)?,
                field(record, 14)?,
            )?;
            Some(Reading::TimeAdjustment {
                timestamp,
                old_timestamp,
                device_id,
            })
        }
        _ => None,
    }
}

fn parse_type2(
    record: &[String],
    device_id: i64,
    timestamp: PrimitiveDateTime,
) -> Option<Reading> {
    let reading_type = field(record, 9)?;
    let value = field(record, 12)?;
    let sport = field(record, 15)? != 0;
    let medication = field(record, 16)? != 0;
    let rapid_flag = field(record, 17)? != 0;
    let long_flag = field(record, 18)? != 0;
    let bitfield = field(record, 19)?;
    let double_long = field(record, 23)?;
    // Food has two encodings on fw 1.5.11: idx 24 = a quick food note with no
    // carb count (the "apple" icon); idx 25 = food *with* a carb count at idx 26
    // (entered in 15 g increments). Both confirmed against real device records.
    let food_no_carbs = field(record, 24)? != 0;
    let food_with_carbs = field(record, 25)? != 0;
    let food = food_no_carbs || food_with_carbs;
    let food_carbs = field(record, 26)?;
    let errors = field(record, 28)?;
    if errors != 0 {
        return None;
    }

    let (is_ketone, source, tag) = match reading_type {
        2 => (false, GlucoseSource::Scan, "(Scan)"),
        0 => (false, GlucoseSource::BloodSample, "(Blood)"),
        1 => (true, GlucoseSource::BloodSample, "(Ketone)"),
        _ => return None,
    };

    let mut comment_parts: Vec<String> = vec![tag.to_string()];

    // Custom comment strings. On fw 1.5.11 these are the 6 trailing fields
    // (idx 32-37), after the 3 firmware-specific fields the reference spec lacks;
    // the bitfield (idx 19) selects which are set, LSB first. (The bitfield→slot
    // mapping is not yet exercised against a real custom-note record.)
    let mut custom_comments = Vec::new();
    for i in 0..6 {
        if bitfield & (1 << i) != 0 {
            if let Some(c) = record.get(32 + i) {
                custom_comments.push(c.trim_matches('"').to_string());
            }
        }
    }
    comment_parts.extend(custom_comments.iter().cloned());

    if sport {
        comment_parts.push("Sport".into());
    }
    if medication {
        comment_parts.push("Medication".into());
    }

    let food_carbs_grams = if food {
        if food_with_carbs && food_carbs != 0 {
            comment_parts.push(format!("Food ({food_carbs} g)"));
            Some(food_carbs as u32)
        } else {
            comment_parts.push("Food".into());
            None
        }
    } else {
        None
    };

    let long_acting_insulin_units = if long_flag {
        let insulin = double_long as f64 / 2.0;
        if insulin != 0.0 {
            comment_parts.push(format!("Long-acting insulin ({insulin:.1})"));
        } else {
            comment_parts.push("Long-acting insulin".into());
        }
        Some(insulin)
    } else {
        None
    };

    // NOTE: reference puts the rapid-insulin *value* at idx 43; on fw 1.5.11 the
    // 3 extra fields likely shift it (~idx 46). Untested — no real rapid-insulin
    // record captured yet — so the flag is detected but the value may read None.
    let rapid_acting_insulin_units = if rapid_flag {
        match field(record, 43) {
            Some(double_rapid) => {
                let units = double_rapid as f64 / 2.0;
                comment_parts.push(format!("Rapid-acting insulin ({units:.1})"));
                Some(units)
            }
            None => {
                comment_parts.push("Rapid-acting insulin".into());
                None
            }
        }
    } else {
        None
    };

    let comment = comment_parts.join("; ");
    let annotations = Annotations {
        sport,
        medication,
        food,
        food_carbs_grams,
        long_acting_insulin: long_flag,
        long_acting_insulin_units,
        rapid_acting_insulin: rapid_flag,
        rapid_acting_insulin_units,
        custom_comments,
    };

    if is_ketone {
        Some(Reading::Ketone {
            timestamp,
            value_mmol_l: value as f64 / 18.0,
            device_id,
            annotations,
            comment,
        })
    } else {
        Some(Reading::Glucose {
            timestamp,
            value_mg_dl: value,
            source,
            device_id,
            annotations,
            comment,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn rec(fields: &[&str]) -> Vec<String> {
        fields.iter().map(|s| s.to_string()).collect()
    }

    /// Build a sparse 46-field record, setting (index, value) pairs.
    fn arresult(pairs: &[(usize, &str)]) -> Vec<String> {
        let mut r = vec!["0".to_string(); 46];
        for &(i, v) in pairs {
            r[i] = v.to_string();
        }
        r
    }

    /// Split a raw CSV line, as `query_multirecord` yields it.
    fn raw(line: &str) -> Vec<String> {
        line.split(',').map(|s| s.to_string()).collect()
    }

    // All expected values below were generated by the reference Python
    // (glucometerutils `_parse_record`/`_parse_arresult`); see
    // `looplace-libre/tools/generate_kat.py` / parse oracle.

    #[test]
    fn history_record() {
        let record = rec(&[
            "12", "0", "6", "19", "26", "8", "34", "0", "0", "0", "0", "0", "0", "105", "0", "0",
        ]);
        let r = parse_history_record(&record).unwrap();
        assert_eq!(
            r,
            Reading::Glucose {
                timestamp: datetime!(2026-06-19 08:34:00),
                value_mg_dl: 105,
                source: GlucoseSource::SensorHistory,
                device_id: 12,
                annotations: Annotations::default(),
                comment: "(Sensor)".into(),
            }
        );
    }

    #[test]
    fn history_record_with_error_is_skipped() {
        let mut record = rec(&[
            "12", "0", "6", "19", "26", "8", "34", "0", "0", "0", "0", "0", "0", "105", "0", "1",
        ]);
        record[15] = "1".into(); // errors != 0
        assert!(parse_history_record(&record).is_none());
    }

    #[test]
    fn arresult_scan_with_food_and_long_insulin() {
        let record = arresult(&[
            (0, "12"), (1, "2"), (2, "6"), (3, "19"), (4, "26"), (5, "9"), (6, "0"), (7, "0"),
            (9, "2"), (12, "120"), (18, "1"), (23, "10"), (25, "1"), (26, "30"), (28, "0"),
        ]);
        let r = parse_arresult_record(&record).unwrap();
        match r {
            Reading::Glucose { value_mg_dl, source, comment, annotations, .. } => {
                assert_eq!(value_mg_dl, 120);
                assert_eq!(source, GlucoseSource::Scan);
                assert_eq!(comment, "(Scan); Food (30 g); Long-acting insulin (5.0)");
                assert_eq!(annotations.food_carbs_grams, Some(30));
                assert_eq!(annotations.long_acting_insulin_units, Some(5.0));
            }
            other => panic!("expected glucose, got {other:?}"),
        }
    }

    #[test]
    fn arresult_blood() {
        let record = arresult(&[
            (0, "12"), (1, "2"), (2, "6"), (3, "19"), (4, "26"), (5, "9"), (6, "5"), (7, "0"),
            (9, "0"), (12, "98"), (28, "0"),
        ]);
        match parse_arresult_record(&record).unwrap() {
            Reading::Glucose { value_mg_dl, source, comment, .. } => {
                assert_eq!(value_mg_dl, 98);
                assert_eq!(source, GlucoseSource::BloodSample);
                assert_eq!(comment, "(Blood)");
            }
            other => panic!("expected glucose, got {other:?}"),
        }
    }

    #[test]
    fn arresult_ketone_converts_to_mmol() {
        let record = arresult(&[
            (0, "12"), (1, "2"), (2, "6"), (3, "19"), (4, "26"), (5, "9"), (6, "10"), (7, "0"),
            (9, "1"), (12, "18"), (28, "0"),
        ]);
        match parse_arresult_record(&record).unwrap() {
            Reading::Ketone { value_mmol_l, comment, .. } => {
                assert_eq!(value_mmol_l, 1.0); // 18 / 18
                assert_eq!(comment, "(Ketone)");
            }
            other => panic!("expected ketone, got {other:?}"),
        }
    }

    #[test]
    fn arresult_scan_with_rapid_insulin() {
        let record = arresult(&[
            (0, "12"), (1, "2"), (2, "6"), (3, "19"), (4, "26"), (5, "9"), (6, "15"), (7, "0"),
            (9, "2"), (12, "110"), (17, "1"), (43, "7"), (28, "0"),
        ]);
        match parse_arresult_record(&record).unwrap() {
            Reading::Glucose { comment, annotations, .. } => {
                assert_eq!(comment, "(Scan); Rapid-acting insulin (3.5)");
                assert_eq!(annotations.rapid_acting_insulin_units, Some(3.5));
            }
            other => panic!("expected glucose, got {other:?}"),
        }
    }

    #[test]
    fn arresult_time_adjustment() {
        let record = arresult(&[
            (0, "12"), (1, "5"), (2, "6"), (3, "19"), (4, "26"), (5, "10"), (6, "0"), (7, "0"),
            (9, "6"), (10, "19"), (11, "26"), (12, "9"), (13, "30"), (14, "0"),
        ]);
        assert_eq!(
            parse_arresult_record(&record).unwrap(),
            Reading::TimeAdjustment {
                timestamp: datetime!(2026-06-19 10:00:00),
                old_timestamp: datetime!(2026-06-19 09:30:00),
                device_id: 12,
            }
        );
    }

    // --- Regression fixtures captured from a real FreeStyle Libre 2 (fw 1.5.11) ---

    #[test]
    fn real_scan_with_food_15g_and_exercise() {
        let r = parse_arresult_record(&raw(
            "453,2,6,19,26,9,11,30,1,2,0,0,94,1,3,1,0,0,0,0,0,0,3,0,0,1,15,0,0,276,94,5,\"\",\"\",\"\",\"\",\"\",\"\"",
        ))
        .unwrap();
        match r {
            Reading::Glucose { timestamp, value_mg_dl, source, comment, annotations, .. } => {
                assert_eq!(timestamp, datetime!(2026-06-19 09:11:30));
                assert_eq!(value_mg_dl, 94);
                assert_eq!(source, GlucoseSource::Scan);
                assert_eq!(comment, "(Scan); Sport; Food (15 g)");
                assert!(annotations.sport);
                assert!(annotations.food);
                assert_eq!(annotations.food_carbs_grams, Some(15));
            }
            other => panic!("expected glucose, got {other:?}"),
        }
    }

    #[test]
    fn real_scan_with_food_no_grams() {
        // The "apple" note: food logged without a carb count (idx 24 = 1).
        let r = parse_arresult_record(&raw(
            "121,2,6,18,26,19,22,38,1,2,0,0,81,1,3,0,0,0,0,0,0,0,3,0,1,0,0,0,0,320,81,-64,\"\",\"\",\"\",\"\",\"\",\"\"",
        ))
        .unwrap();
        match r {
            Reading::Glucose { value_mg_dl, comment, annotations, .. } => {
                assert_eq!(value_mg_dl, 81);
                assert_eq!(comment, "(Scan); Food");
                assert!(annotations.food);
                assert_eq!(annotations.food_carbs_grams, None);
                assert!(!annotations.sport);
            }
            other => panic!("expected glucose, got {other:?}"),
        }
    }

    #[test]
    fn real_plain_scan_has_no_annotations() {
        let r = parse_arresult_record(&raw(
            "440,2,6,19,26,8,59,41,1,2,0,0,85,1,3,0,0,0,0,0,0,0,3,0,0,0,0,0,0,277,85,-35,\"\",\"\",\"\",\"\",\"\",\"\"",
        ))
        .unwrap();
        match r {
            Reading::Glucose { value_mg_dl, comment, annotations, .. } => {
                assert_eq!(value_mg_dl, 85);
                assert_eq!(comment, "(Scan)");
                assert_eq!(annotations, Annotations::default());
            }
            other => panic!("expected glucose, got {other:?}"),
        }
    }

    #[test]
    fn real_error_record_is_skipped() {
        // error-bitfield (idx 28) = 32768 (0x8000) → invalid reading, skipped.
        assert!(parse_arresult_record(&raw(
            "93,2,6,18,26,18,9,43,1,2,0,1,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32768,999,0,0,\"\",\"\",\"\",\"\",\"\",\"\"",
        ))
        .is_none());
    }
}

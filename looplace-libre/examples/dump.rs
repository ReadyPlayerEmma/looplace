// SPDX-License-Identifier: Apache-2.0
//! Dump all readings from a physically attached FreeStyle Libre 2 reader.
//!
//! - stdout: a CSV with explicit annotation columns (food, carbs, sport, …).
//! - stderr: a summary plus the *raw* `$arresult?` records (the annotated ones),
//!   so the on-device field values can be inspected directly.
//!
//! ```text
//! cargo run -p looplace-libre --features transport,libre2-keys --example dump > readings.csv
//! ```
//!
//! Read-only — never writes to the device.

#[cfg(all(feature = "transport", feature = "libre2-keys"))]
fn main() {
    use looplace_libre::records::{
        parse_arresult_record, parse_history_record, Annotations, Reading,
    };
    use looplace_libre::LibreDevice;
    use time::macros::format_description;

    let fmt = format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
    let ts = |t: &time::PrimitiveDateTime| t.format(&fmt).unwrap_or_default();

    let b = |x: bool| if x { "1" } else { "0" };
    // food,carbs_g,sport,medication,rapid_u,long_u,custom
    let ann_cols = |a: &Annotations| {
        format!(
            "{},{},{},{},{},{},{}",
            b(a.food),
            a.food_carbs_grams.map(|g| g.to_string()).unwrap_or_default(),
            b(a.sport),
            b(a.medication),
            a.rapid_acting_insulin_units.map(|u| format!("{u:.1}")).unwrap_or_default(),
            a.long_acting_insulin_units.map(|u| format!("{u:.1}")).unwrap_or_default(),
            a.custom_comments.join("|"),
        )
    };
    let empty_ann = Annotations::default();
    let csv_row = |r: &Reading| -> String {
        match r {
            Reading::Glucose { timestamp, value_mg_dl, source, device_id, annotations, comment } => {
                format!(
                    "{},glucose,{},mg/dL,{:?},{},{},{}",
                    ts(timestamp), value_mg_dl, source, device_id, ann_cols(annotations),
                    comment.replace(',', ";")
                )
            }
            Reading::Ketone { timestamp, value_mmol_l, device_id, annotations, comment } => {
                format!(
                    "{},ketone,{},mmol/L,,{},{},{}",
                    ts(timestamp), value_mmol_l, device_id, ann_cols(annotations),
                    comment.replace(',', ";")
                )
            }
            Reading::TimeAdjustment { timestamp, old_timestamp, device_id } => {
                format!(
                    "{},time_adjustment,,,from={},{},{},",
                    ts(timestamp), ts(old_timestamp), device_id, ann_cols(&empty_ann)
                )
            }
        }
    };

    eprintln!("Opening FreeStyle Libre 2 reader …");
    let mut device = match LibreDevice::open_libre2() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("✗ open failed: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = device.connect() {
        eprintln!("✗ connect/handshake failed: {e}");
        std::process::exit(2);
    }
    let serial = device.serial_number().unwrap_or_else(|_| "?".into());
    let version = device.software_version().unwrap_or_else(|_| "?".into());
    let unit = device.glucose_unit().ok();
    eprintln!("Reader {serial} (fw {version}), unit {unit:?}.");

    println!("timestamp,kind,value,unit,source,seq,food,carbs_g,sport,medication,rapid_u,long_u,custom,comment");

    let session = device.session_mut();

    // History: background CGM trace (no annotations possible).
    let history = session.query_multirecord(b"$history?").unwrap_or_default();
    let mut glucose = 0u32;
    for row in &history {
        if let Some(r) = parse_history_record(row) {
            glucose += 1;
            println!("{}", csv_row(&r));
        }
    }

    // Arresult: scans / blood / ketone / clock changes — where annotations live.
    let arresult = session.query_multirecord(b"$arresult?").unwrap_or_default();
    eprintln!("\n--- raw $arresult? records ({}) [the annotated ones] ---", arresult.len());
    let (mut scans, mut ketone, mut adjust) = (0u32, 0u32, 0u32);
    for (i, row) in arresult.iter().enumerate() {
        // Decode the key annotation indices straight from the raw fields.
        let g = |idx: usize| row.get(idx).map(String::as_str).unwrap_or("-");
        eprintln!(
            "#{i:<2} type={} rtype={} val={} food24={} food25={} carbs={} sport={} med={} rapid={} long={} | nfields={}",
            g(1), g(9), g(12), g(24), g(25), g(26), g(15), g(16), g(17), g(18), row.len()
        );
        eprintln!("    raw: {}", row.join(","));

        if let Some(r) = parse_arresult_record(row) {
            match r {
                Reading::Glucose { .. } => scans += 1,
                Reading::Ketone { .. } => ketone += 1,
                Reading::TimeAdjustment { .. } => adjust += 1,
            }
            println!("{}", csv_row(&r));
        }
    }

    eprintln!(
        "\n✓ {} history + {} scan/blood + {ketone} ketone + {adjust} time-adjust.",
        glucose, scans
    );
}

#[cfg(not(all(feature = "transport", feature = "libre2-keys")))]
fn main() {
    eprintln!(
        "dump needs the `transport` and `libre2-keys` features and a connected reader:\n\
         \n    cargo run -p looplace-libre --features transport,libre2-keys --example dump > readings.csv\n"
    );
    std::process::exit(2);
}

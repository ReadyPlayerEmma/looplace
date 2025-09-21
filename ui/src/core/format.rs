//! Formatting helpers for presenting metrics.

pub fn format_ms(value: f64) -> String {
    if !value.is_finite() {
        "—".to_string()
    } else {
        format!("{value:.0} ms")
    }
}

pub fn format_slope(value: f64) -> String {
    if !value.is_finite() {
        "—".to_string()
    } else {
        format!("{value:.2} ms/min")
    }
}

pub fn format_percent(value: f64) -> String {
    if !value.is_finite() {
        "—".to_string()
    } else {
        format!("{:.0}%", value * 100.0)
    }
}

pub fn format_number(value: f64, decimals: usize) -> String {
    if !value.is_finite() {
        return "—".to_string();
    }

    format!("{value:.prec$}", value = value, prec = decimals)
}

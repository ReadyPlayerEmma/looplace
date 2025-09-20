//! Formatting helpers for presenting metrics.

pub fn format_ms(value: f64) -> String {
    format!("{value:.0} ms")
}

pub fn format_slope(value: f64) -> String {
    format!("{value:.2} ms/min")
}

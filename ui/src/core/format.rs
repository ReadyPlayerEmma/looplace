//! Formatting helpers for presenting metrics.

pub fn format_ms(value: f32) -> String {
    format!("{value:.0} ms")
}

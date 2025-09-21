//! High-resolution timing helpers usable in both WASM and native targets.

#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

#[cfg(target_arch = "wasm32")]
pub type InstantStamp = f64;

#[cfg(not(target_arch = "wasm32"))]
pub type InstantStamp = std::time::Instant;

/// Return the current high-resolution timestamp in milliseconds (WASM) or an `Instant` (native).
#[cfg(target_arch = "wasm32")]
pub fn now() -> InstantStamp {
    web_sys::window()
        .and_then(|win| win.performance())
        .map(|perf| perf.now())
        .unwrap_or_else(|| js_sys::Date::now())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn now() -> InstantStamp {
    std::time::Instant::now()
}

/// Convert a timestamp difference to milliseconds as `f64`.
#[cfg(target_arch = "wasm32")]
pub fn duration_ms(start: InstantStamp, end: InstantStamp) -> f64 {
    (end - start).max(0.0)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn duration_ms(start: InstantStamp, end: InstantStamp) -> f64 {
    end.checked_duration_since(start)
        .unwrap_or(Duration::from_millis(0))
        .as_secs_f64()
        * 1000.0
}

/// Convenience to obtain milliseconds since a reference point.
#[cfg(target_arch = "wasm32")]
pub fn elapsed_ms(start: InstantStamp) -> f64 {
    duration_ms(start, now())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn elapsed_ms(start: InstantStamp) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

/// Cross-platform async sleep in milliseconds.
#[cfg(target_arch = "wasm32")]
pub async fn sleep_ms(amount: u64) {
    use gloo_timers::future::TimeoutFuture;

    TimeoutFuture::new(amount as u32).await;
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn sleep_ms(amount: u64) {
    tokio::time::sleep(Duration::from_millis(amount)).await;
}

/// Convert milliseconds into minutes as `f64`.
pub fn ms_to_minutes(ms: f64) -> f64 {
    ms / 1000.0 / 60.0
}

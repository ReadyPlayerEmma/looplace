//! Platform detection helpers used for analytics and storage metadata.

use std::future::Future;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    Web,
    Desktop,
    Mobile,
    Unknown,
}

impl Platform {
    pub fn as_str(&self) -> &'static str {
        match self {
            Platform::Web => "web",
            Platform::Desktop => "desktop",
            Platform::Mobile => "mobile",
            Platform::Unknown => "unknown",
        }
    }
}

pub fn current() -> Platform {
    detect_platform()
}

pub fn platform_string() -> String {
    current().as_str().to_string()
}

pub fn timezone_string() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let offset_minutes = js_sys::Date::new_0().get_timezone_offset().round() as i32;
        format_utc_offset(-offset_minutes)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        // TODO: capture the actual local offset on desktop targets.
        "UTC".to_string()
    }
}

pub fn user_agent_string() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window().and_then(|win| win.navigator().user_agent().ok())
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        None
    }
}

#[cfg(target_arch = "wasm32")]
pub fn spawn_future<F>(future: F)
where
    F: Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_future<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    let _ = tokio::spawn(future);
}

fn detect_platform() -> Platform {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(ua) = user_agent_string() {
            let ua_lower = ua.to_lowercase();
            if ua_lower.contains("iphone")
                || ua_lower.contains("android")
                || ua_lower.contains("ipad")
                || ua_lower.contains("mobile")
            {
                return Platform::Mobile;
            }
            return Platform::Web;
        }
        Platform::Web
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        Platform::Desktop
    }
}

#[cfg(target_arch = "wasm32")]
fn format_utc_offset(offset_minutes: i32) -> String {
    let hours = offset_minutes / 60;
    let minutes = (offset_minutes.abs() % 60) as i32;
    if offset_minutes == 0 {
        "UTC".to_string()
    } else {
        format!("UTC{:+03}:{:02}", hours, minutes)
    }
}

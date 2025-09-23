#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(feature = "desktop")]
use std::path::PathBuf;

#[cfg(feature = "desktop")]
use dioxus::desktop::Config;
use dioxus::prelude::*;

use ui::components::Navbar;
use ui::views::{Home, NBack2, Pvt, Results};

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(DesktopNavbar)]
    #[route("/")]
    Home {},
    #[route("/test/pvt")]
    Pvt {},
    #[route("/test/nback")]
    NBack2 {},
    #[route("/results")]
    Results {},
}

const MAIN_CSS: Asset = asset!("/assets/main.css");
const MAIN_CSS_INLINE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/main.css"));

#[cfg(feature = "desktop")]
fn main() {
    let resource_dir = resolve_resource_dir();

    LaunchBuilder::desktop()
        .with_cfg(Config::new().with_resource_directory(resource_dir))
        .launch(App);
}

#[cfg(all(feature = "server", not(feature = "desktop")))]
fn main() {
    LaunchBuilder::server().launch(App);
}

#[component]
fn App() -> Element {
    // Build cool things ✌️

    rsx! {
        // Global app resources
        document::Link { rel: "stylesheet", href: MAIN_CSS }

        if cfg!(not(debug_assertions)) {
            document::Style { "{MAIN_CSS_INLINE}" }
        }

        Router::<Route> {}
    }
}

#[cfg(feature = "desktop")]
fn resolve_resource_dir() -> PathBuf {
    #[cfg(debug_assertions)]
    {
        // During `cargo run` / `dx serve` load directly from the crate.
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/assets"))
    }

    #[cfg(not(debug_assertions))]
    {
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join("assets")))
            .unwrap_or_else(|| PathBuf::from("assets"))
    }
}

/// A desktop-specific Router around the shared `Navbar` component
/// which allows us to use the desktop-specific `Route` enum.
#[component]
fn DesktopNavbar() -> Element {
    rsx! {
        Navbar {
            div {
                class: "navbar__brand",
                Link {
                    class: "navbar__brand-link",
                    to: Route::Home {},
                    span { class: "navbar__brand-spark", aria_hidden: "true" }
                    span { class: "navbar__brand-mark", "Looplace" }
                }
                span { class: "navbar__brand-subtitle", "Track focus with compassion" }
            }
            nav {
                class: "navbar__links",
                Link {
                    class: "navbar__link",
                    to: Route::Home {},
                    "Home"
                }
                Link {
                    class: "navbar__link",
                    to: Route::Pvt {},
                    "PVT"
                }
                Link {
                    class: "navbar__link",
                    to: Route::NBack2 {},
                    "2-back"
                }
                Link {
                    class: "navbar__link",
                    to: Route::Results {},
                    "Results"
                }
            }
        }

        Outlet::<Route> {}
    }
}

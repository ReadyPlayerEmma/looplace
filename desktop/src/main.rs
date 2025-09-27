#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(feature = "desktop")]
use std::path::PathBuf;

#[cfg(feature = "desktop")]
use dioxus::desktop::Config;
use dioxus::prelude::*;

use ui::components::app_navbar::{register_nav, NavBuilder};
use ui::components::AppNavbar;

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

    // Increase default window size (~20% larger than a common 960x600 baseline → 1152x720).
    // We extend the existing Config with a window builder specifying the inner size.
    LaunchBuilder::desktop()
        .with_cfg(
            Config::new().with_resource_directory(resource_dir), // Removed explicit window sizing (was causing build/type issues). Use default size for now.
        )
        .launch(App);
}

#[cfg(all(feature = "server", not(feature = "desktop")))]
fn main() {
    LaunchBuilder::server().launch(App);
}

fn nav_home(label: &str) -> Element {
    rsx!(Link { class: "navbar__link", to: Route::Home {}, "{label}" })
}
fn nav_pvt(label: &str) -> Element {
    rsx!(Link { class: "navbar__link", to: Route::Pvt {}, "{label}" })
}
fn nav_nback(label: &str) -> Element {
    rsx!(Link { class: "navbar__link", to: Route::NBack2 {}, "{label}" })
}
fn nav_results(label: &str) -> Element {
    rsx!(Link { class: "navbar__link", to: Route::Results {}, "{label}" })
}

#[component]
fn App() -> Element {
    {
        ui::i18n::init();
        // Register localized navigation builder (desktop)
        register_nav(NavBuilder {
            home: nav_home,
            pvt: nav_pvt,
            nback: nav_nback,
            results: nav_results,
        });
    } // Build cool things ✌️

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
        AppNavbar { }

        Outlet::<Route> {}
    }
}

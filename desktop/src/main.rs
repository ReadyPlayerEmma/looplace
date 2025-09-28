#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(feature = "desktop")]
use std::path::PathBuf;

#[cfg(feature = "desktop")]
use dioxus::desktop::{tao::window::WindowBuilder, Config};
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

const MAIN_CSS_INLINE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../ui/assets/theme/main.css"
)); // Embedded shared theme (ui/assets/theme/main.css); no separate desktop /assets needed.

#[cfg(feature = "desktop")]
fn main() {
    let resource_dir = resolve_resource_dir();

    // Maximize window on launch (dioxus-desktop 0.6.x: pass a WindowBuilder value)
    LaunchBuilder::desktop()
        .with_cfg(
            Config::new()
                .with_window(
                    WindowBuilder::new()
                        .with_title(format!("Looplace – v{}", env!("CARGO_PKG_VERSION")))
                        .with_maximized(true),
                )
                .with_resource_directory(resource_dir),
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
    // Initialize i18n once
    ui::i18n::init();

    // Provide global reactive language code signal (mirrors web approach)
    // AppNavbar (shared) will update this via context on language selection.
    let lang_code = use_signal(|| "en-US".to_string());
    use_context_provider(|| lang_code);

    // Register localized navigation builder (desktop)
    register_nav(NavBuilder {
        home: nav_home,
        pvt: nav_pvt,
        nback: nav_nback,
        results: nav_results,
    });

    // Runtime maximize fallback (in case initial builder maximize is ignored by WM)
    #[cfg(feature = "desktop")]
    {
        let win = dioxus::desktop::use_window();
        use_effect(move || {
            win.set_maximized(true);
        });
    }

    rsx! {
        // Global app resources
        // Always inline embedded CSS (no external file dependency for desktop builds)
        document::Style { "{MAIN_CSS_INLINE}" }

        // Key the routed subtree by current language to force full remount on change
        // Hidden marker keeps explicit reactive dependency (optional)
        div { style: "display:none", "lang={lang_code()}" }
        // Keyed wrapper div to force full remount on language change and include a hidden
        // reactive marker so we always depend on the lang_code signal.
        div {
            key: "{lang_code()}",
            div { style: "display:none", "{lang_code()}" }
            Router::<Route> { }
        }
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

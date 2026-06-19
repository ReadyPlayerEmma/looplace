#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(feature = "desktop")]
use std::path::PathBuf;

#[cfg(feature = "desktop")]
use dioxus::desktop::{tao::window::WindowBuilder, Config};
use dioxus::prelude::*;

use ui::components::app_navbar::{register_nav, NavBuilder};
use ui::components::AppNavbar;

use ui::views::{Glucose, Home, NBack2, Pvt, Results};

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
    #[route("/glucose")]
    Glucose {},
}

const MAIN_CSS_INLINE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../ui/assets/theme/main.css"
)); // Embedded shared theme (ui/assets/theme/main.css); no separate desktop /assets needed.

#[cfg(feature = "desktop")]
fn main() {
    // Relocate WebView2 user data folder to OS app data dir so the install folder stays clean.
    // Done early so WebView2 picks it up before the webview environment is created.
    #[cfg(windows)]
    {
        use std::path::PathBuf;
        if let Some(base) = std::env::var_os("LOCALAPPDATA") {
            let mut dir = PathBuf::from(base);
            dir.push("Looplace");
            dir.push("webview2");
            if std::fs::create_dir_all(&dir).is_ok() {
                std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &dir);
            }
        }
    }

    let resource_dir = resolve_resource_dir();

    // One-time cognition→store upgrade on startup (backs up summaries.json first;
    // idempotent). Runs before the UI; logs and continues on any error.
    init_health_store();

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

/// One-time cognition→store upgrade + open the native health store. Logs and
/// continues on any error — never blocks app launch. The Parquet store lives in
/// the same per-user data dir as the legacy `summaries.json`.
#[cfg(feature = "desktop")]
fn init_health_store() {
    use looplace_store::migrate::{run_upgrade, MigrationOutcome, MigrationPlan};
    use looplace_store::ParquetStore;

    let data_dir = match ui::core::storage::data_dir() {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("[store] data dir unavailable, skipping migration: {err}");
            return;
        }
    };

    let mut store = match ParquetStore::open(data_dir.join("looplace.parquet")) {
        Ok(store) => store,
        Err(err) => {
            eprintln!("[store] could not open health store: {err}");
            return;
        }
    };

    let tag = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let plan = MigrationPlan::for_data_dir(&data_dir, &tag);
    match run_upgrade(&plan, &mut store) {
        Ok(MigrationOutcome::Migrated(r)) => eprintln!(
            "[store] migrated {} cognition sessions ({} skipped) → {} observations",
            r.sessions, r.skipped_records, r.observations_inserted
        ),
        Ok(MigrationOutcome::AlreadyDone) => eprintln!("[store] cognition already migrated"),
        Ok(MigrationOutcome::NothingToMigrate) => {
            eprintln!("[store] no legacy cognition data to migrate")
        }
        Err(err) => eprintln!("[store] migration error (will retry next launch): {err}"),
    }
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
fn nav_glucose(label: &str) -> Element {
    rsx!(Link { class: "navbar__link", to: Route::Glucose {}, "{label}" })
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
        glucose: nav_glucose,
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

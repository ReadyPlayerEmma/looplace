use dioxus::prelude::*;

use ui::components::app_navbar::{register_nav, NavBuilder};
use ui::components::AppNavbar;
use ui::views::{Home, NBack2, Pvt, Results};

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(WebNavbar)]
    #[route("/")]
    Home {},
    #[route("/test/pvt")]
    Pvt {},
    #[route("/test/nback")]
    NBack2 {},
    #[route("/results")]
    Results {},
}

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css"); // NOTE: Currently referencing the web copy for HTTP caching. Planned future switch to unified shared theme (ui/assets/theme/main.css) once cache + versioning strategy is in place.

fn nav_home(label: &str) -> Element {
    rsx!(Link {
        class: "navbar__link",
        to: Route::Home {},
        "{label}"
    })
}
fn nav_pvt(label: &str) -> Element {
    rsx!(Link {
        class: "navbar__link",
        to: Route::Pvt {},
        "{label}"
    })
}
fn nav_nback(label: &str) -> Element {
    rsx!(Link {
        class: "navbar__link",
        to: Route::NBack2 {},
        "{label}"
    })
}
fn nav_results(label: &str) -> Element {
    rsx!(Link {
        class: "navbar__link",
        to: Route::Results {},
        "{label}"
    })
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    // Initialize i18n once
    ui::i18n::init();

    // Global reactive language code signal.
    // Keying the routed subtree by this value guarantees a full remount
    // so all `t!()` calls re-evaluate immediately on locale change.
    let lang_code = use_signal(|| "en-US".to_string());
    // Debug: log initial language code when App renders
    #[cfg(debug_assertions)]
    {
        // Simple stdout log (avoid requiring logger setup)
        println!("[i18n] App initial render lang_code={}", lang_code());
    }
    use_context_provider(|| lang_code);

    // Register localized navigation builder (Option A)
    register_nav(NavBuilder {
        home: nav_home,
        pvt: nav_pvt,
        nback: nav_nback,
        results: nav_results,
    });

    rsx! {
        // Global app resources
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }

        // Keyed wrapper (forces full remount) + hidden marker (reactive dependency)
        div {
            key: "{lang_code()}",
            // Hidden marker to ensure dependency on lang_code even if Router optimizes internally
            div { style: "display:none", "{lang_code()}" }
            Router::<Route> { }
        }
    }
}

/// A web-specific Router around the shared `Navbar` component
/// which allows us to use the web-specific `Route` enum.
#[component]
fn WebNavbar() -> Element {
    // Consume language code (if provided) so the navbar's select can update it
    // and force a dependency so the layout re-renders on language change.
    let lang_code_ctx: Option<Signal<String>> = try_use_context::<Signal<String>>();
    let _lang_marker = lang_code_ctx.as_ref().map(|s| s()).unwrap_or_default();

    rsx! {
        // Hidden marker nodes ensure this layout depends on the language code
        div { style: "display:none", "lang={_lang_marker}" }
        AppNavbar { }
        Outlet::<Route> {}
    }
}

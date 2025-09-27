use crate::i18n::{self};
use crate::t;
use dioxus::prelude::*;
use once_cell::sync::OnceCell;

// Navbar stylesheet (mirrors legacy Navbar so styling applies here too)
const NAVBAR_CSS: Asset = asset!("/assets/styling/navbar.css");
const NAVBAR_CSS_INLINE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/styling/navbar.css"
));

/// Option A infrastructure:
/// Platforms can (optionally) register a `NavBuilder` providing fully constructed
/// `Link` elements (so `ui` does not need to know each platform's `Route` enum).
///
/// If a builder is registered, `AppNavbar` renders localized labels *inside* each
/// supplied link (it ignores any pre-existing child text).
///
/// If no builder is registered, we fall back to any raw `children` passed (legacy)
/// so existing code does not break while platforms migrate.
///
/// Migration steps for a platform crate (desktop/web):
/// 1. Define a function returning `NavBuilder` where each closure constructs a
///    `Link { to: Route::..., class: "navbar__link", ... }`.
/// 2. Call `ui::components::app_navbar::register_nav(builder)` before rendering
///    the root (e.g. at top of `App()`).
/// 3. Use `AppNavbar {}` with no manual nav link children.
///
/// Example (in platform crate):
/// ```ignore
/// use ui::components::app_navbar::{NavBuilder, register_nav};
/// fn install_nav() {
///     register_nav(NavBuilder {
///         home: || rsx!( Link { class: "navbar__link", to: Route::Home {} } ),
///         pvt: || rsx!( Link { class: "navbar__link", to: Route::Pvt {} } ),
///         nback: || rsx!( Link { class: "navbar__link", to: Route::NBack2 {} } ),
///         results: || rsx!( Link { class: "navbar__link", to: Route::Results {} } ),
///     });
/// }
/// ```
///
/// The language selector triggers a re-render via a local signal; every render
/// pulls fresh localized strings via `fl!`.
///
/// NOTE: We do *not* attempt to mutate platform-supplied `Link` children; instead
/// we re-render our own internal nav when a builder is present.
pub struct NavBuilder {
    // Each closure must return a Link (or element styled as a nav link) whose
    // children will be exactly the localized label string passed in.
    pub home: fn(label: &str) -> Element,
    pub pvt: fn(label: &str) -> Element,
    pub nback: fn(label: &str) -> Element,
    pub results: fn(label: &str) -> Element,
}

static NAV_BUILDER: OnceCell<NavBuilder> = OnceCell::new();

pub fn register_nav(builder: NavBuilder) {
    let _ = NAV_BUILDER.set(builder);
}

#[component]
pub fn AppNavbar(children: Element) -> Element {
    i18n::init();

    let mut current_lang = use_signal(|| "en-US".to_string());
    let langs = use_signal(i18n::available_languages);
    let show_switcher = langs().len() > 1;
    // Obtain global language code signal if the platform (web crate) provided it.
    let lang_code_ctx: Option<Signal<String>> = try_use_context::<Signal<String>>();
    // Establish a reactive dependency on the global language code (if provided)
    let _lang_marker = lang_code_ctx.as_ref().map(|c| c()).unwrap_or_default();

    #[cfg(debug_assertions)]
    {
        if let Some(code) = lang_code_ctx.as_ref() {
            println!("[i18n] AppNavbar render lang={}", code());
        } else {
            println!("[i18n] AppNavbar render lang=<none>");
        }
    }

    let on_change = move |evt: dioxus::events::FormEvent| {
        let val = evt.value();
        if i18n::set_language(&val).is_ok() {
            // Update local select state
            current_lang.set(val.clone());
            // Propagate to global language code signal if the platform provided one
            if let Some(mut code) = lang_code_ctx {
                code.set(val);
            }
        }
    };

    // Build internal localized nav if a NavBuilder is registered.
    // New contract: each closure receives the localized label & returns a Link
    // that already *contains* that label as its child, preserving styling.
    let internal_nav: Option<VNode> = NAV_BUILDER.get().map(|b| {
        let home = (b.home)(&t!("nav-home"));
        let pvt = (b.pvt)(&t!("nav-pvt"));
        let nback = (b.nback)(&t!("nav-nback"));
        let results = (b.results)(&t!("nav-results"));

        rsx! {
            nav { class: "navbar__links",
                {home}
                {pvt}
                {nback}
                {results}
            }
        }
        .expect("AppNavbar: rsx render failed")
    });

    let tagline = t!("tagline");

    rsx! {
        // Include shared navbar stylesheet (and inline in release native)
        document::Link { rel: "stylesheet", href: NAVBAR_CSS }
        if cfg!(all(not(debug_assertions), not(target_arch = "wasm32"))) {
            document::Style { "{NAVBAR_CSS_INLINE}" }
        }

        header {
            id: "navbar",
            class: "navbar",
            // Hidden marker ensures AppNavbar re-renders when the global language signal changes.
            div { style: "display:none", "{_lang_marker}" }
            div { class: "navbar__inner",
                // Brand
                div { class: "navbar__brand",
                    span { class: "navbar__brand-link",
                        span { class: "navbar__brand-spark", aria_hidden: "true" }
                        span { class: "navbar__brand-mark", "Looplace" }
                    }
                    span { class: "navbar__brand-subtitle", "{tagline}" }
                }

                // Navigation (internal builder or legacy children)
                if let Some(nav) = internal_nav {
                    {nav}
                } else {
                    nav { class: "navbar__links", {children} }
                }

                // Locale switcher
                if show_switcher {
                    div { class: "navbar__locale",
                        label {
                            class: "visually-hidden",
                            r#for: "locale-select",
                            {t!("nav-language-label")}
                        }
                        select {
                            id: "locale-select",
                            value: "{current_lang()}",
                            oninput: on_change,
                            { langs().iter().map(|code| {
                                let c = code.clone();
                                rsx!{
                                    option { key: "{c}", value: "{c}", "{c}" }
                                }
                            })}
                        }
                    }
                }
            }
        }
    }
}

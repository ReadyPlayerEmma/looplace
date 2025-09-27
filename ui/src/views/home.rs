use dioxus::prelude::*;

#[cfg(debug_assertions)]
fn log_home_render(lang: &str) {
    // Lightweight render trace for diagnosing i18n refresh issues.
    println!("[i18n] Home render (lang_marker={lang})");
}

#[component]
pub fn Home() -> Element {
    // Subscribe to global language code (if provided) so we re-render on change.
    let _lang_code: Option<Signal<String>> = try_use_context::<Signal<String>>();
    let _lang_current = _lang_code
        .as_ref()
        .map(|s| s())
        .unwrap_or_else(|| "en-US".to_string());

    // Debug render log
    #[cfg(debug_assertions)]
    {
        log_home_render(&_lang_current);
    }

    rsx! {
        section { class: "page page-home",
            h1 { {crate::t!("home-title")} }
            p { {crate::t!("home-tagline-short")} }
            p { {crate::t!("home-intro-1")} }


            ul { class: "page-home__features",
                li { {crate::t!("home-feature-pvt")} }
                li { {crate::t!("home-feature-nback")} }
                li { {crate::t!("home-feature-local")} }
            }
            p { class: "page-home__cta",
                {crate::t!("home-cta")}
            }
        }
    }
}

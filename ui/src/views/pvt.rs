use dioxus::prelude::*;

use crate::tasks::pvt::PvtView;

#[component]
pub fn Pvt() -> Element {
    // Subscribe to global language code (if provided) so this view re-renders
    // immediately when the locale changes elsewhere (e.g. while on Results).
    let _lang_code: Option<Signal<String>> = try_use_context::<Signal<String>>();
    // Reactive dependency (cloned value) used in a hidden marker node below.
    let _lang_marker = _lang_code.as_ref().map(|s| s()).unwrap_or_default();

    rsx! {
        // Hidden marker node ensures reactive dependency on language signal.
        div { style: "display:none", "{_lang_marker}" }
        section { class: "page page-pvt",
            h1 { {crate::t!("page-pvt-title")} }
            p { {crate::t!("page-pvt-intro")} }
            PvtView {}
        }
    }
}

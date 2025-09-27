use dioxus::prelude::*;

use crate::tasks::nback::NBackView;

#[component]
pub fn NBack2() -> Element {
    // Subscribe to global language code (if provided) so this view re-renders
    // when the user switches language elsewhere (e.g. while staying on this page).
    let _lang_code: Option<Signal<String>> = try_use_context::<Signal<String>>();
    let _lang_marker = _lang_code.as_ref().map(|s| s()).unwrap_or_default();

    rsx! {
        // Hidden marker node retains reactive dependency on language signal.
        div { style: "display:none", "{_lang_marker}" }
        section { class: "page page-nback",
            h1 { {crate::t!("page-nback-title")} }
            p { {crate::t!("page-nback-intro")} }
            NBackView {}
        }
    }
}

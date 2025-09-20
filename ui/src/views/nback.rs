use dioxus::prelude::*;

use crate::tasks::nback::NBackView;

#[component]
pub fn NBack2() -> Element {
    rsx! {
        section { class: "page page-nback",
            h1 { "2-back Working Memory" }
            p {
                "Cycle through balanced letter streams and track hits, misses, and reaction times."
            }
            NBackView {}
        }
    }
}

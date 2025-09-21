use dioxus::prelude::*;

use crate::tasks::nback::NBackView;

#[component]
pub fn NBack2() -> Element {
    rsx! {
        section { class: "page page-nback",
            h1 { "2-back Working Memory" }
            p {
                "Start with a short guided warm-up, then follow the 2-back stream—respond whenever the letter matches the one from two trials ago to capture sensitivity (d′), response bias, and reaction-time trends."
            }
            NBackView {}
        }
    }
}

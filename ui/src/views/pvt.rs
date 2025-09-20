use dioxus::prelude::*;

use crate::tasks::pvt::PvtView;

#[component]
pub fn Pvt() -> Element {
    rsx! {
        section { class: "page page-pvt",
            h1 { "Psychomotor Vigilance Task" }
            p {
                "Run a short vigilance block to capture reaction time metrics and lapse counts."
            }
            PvtView {}
        }
    }
}

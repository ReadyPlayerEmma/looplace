use dioxus::prelude::*;

#[component]
pub fn Home() -> Element {
    rsx! {
        section { class: "page page-home",
            h1 { "Looplace" }
            p { "Small loops • clear minds." }
            p {
                "Track psychomotor vigilance and working memory with shared engines that run everywhere."
            }
            ul { class: "page-home__features",
                li { "Precise PVT timing with local metrics" }
                li { "2-back working memory sessions" }
                li { "Results sync-free: stored locally with export paths" }
            }
            p { class: "page-home__cta",
                "Choose a task to get started."
            }
        }
    }
}

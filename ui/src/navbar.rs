use dioxus::prelude::*;

const NAVBAR_CSS: Asset = asset!("/assets/styling/navbar.css");
const NAVBAR_CSS_INLINE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/styling/navbar.css"
));

#[component]
pub fn Navbar(children: Element) -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: NAVBAR_CSS }
        if cfg!(all(not(debug_assertions), not(target_arch = "wasm32"))) {
            document::Style { "{NAVBAR_CSS_INLINE}" }
        }

        div {
            id: "navbar",
            {children}
        }
    }
}

use dioxus::prelude::*;

const NAVBAR_CSS: Asset = asset!("/assets/styling/navbar.css");
const NAVBAR_CSS_INLINE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/styling/navbar.css"
));

#[component]
pub fn Navbar(children: Element) -> Element {
    // Legacy passthrough (kept for compatibility). Just forwards children.
    rsx! { {children} }
}

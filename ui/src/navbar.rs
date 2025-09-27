use dioxus::prelude::*;

#[component]
pub fn Navbar(children: Element) -> Element {
    // Legacy passthrough (kept for compatibility). Just forwards children.
    rsx! { {children} }
}

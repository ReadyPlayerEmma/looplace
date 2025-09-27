use dioxus::prelude::*;

use ui::components::app_navbar::{register_nav, NavBuilder};
use ui::components::AppNavbar;
use ui::i18n;
use ui::views::{Home, NBack2, Pvt, Results};

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(WebNavbar)]
    #[route("/")]
    Home {},
    #[route("/test/pvt")]
    Pvt {},
    #[route("/test/nback")]
    NBack2 {},
    #[route("/results")]
    Results {},
}

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");

fn nav_home(label: &str) -> Element {
    rsx!(Link {
        class: "navbar__link",
        to: Route::Home {},
        "{label}"
    })
}
fn nav_pvt(label: &str) -> Element {
    rsx!(Link {
        class: "navbar__link",
        to: Route::Pvt {},
        "{label}"
    })
}
fn nav_nback(label: &str) -> Element {
    rsx!(Link {
        class: "navbar__link",
        to: Route::NBack2 {},
        "{label}"
    })
}
fn nav_results(label: &str) -> Element {
    rsx!(Link {
        class: "navbar__link",
        to: Route::Results {},
        "{label}"
    })
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    {
        ui::i18n::init();
        // Register localized navigation builder (Option A)
        register_nav(NavBuilder {
            home: nav_home,
            pvt: nav_pvt,
            nback: nav_nback,
            results: nav_results,
        });
    }

    rsx! {
        // Global app resources
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }

        Router::<Route> {}
    }
}

/// A web-specific Router around the shared `Navbar` component
/// which allows us to use the web-specific `Route` enum.
#[component]
fn WebNavbar() -> Element {
    rsx! {
        AppNavbar { }
        Outlet::<Route> {}
    }
}

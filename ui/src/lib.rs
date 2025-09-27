//! Shared UI crate for Looplace. Most cross-platform logic and views live here.

pub mod core;
pub mod i18n;
pub mod results;
pub mod tasks;
pub mod views;

mod navbar;
pub mod components {
    // Localized application navbar (components/app_navbar.rs)
    pub mod app_navbar;
    pub use app_navbar::register_nav;
    pub use app_navbar::AppNavbar;
    pub use app_navbar::NavBuilder;

    // Legacy minimalist Navbar passthrough (ui/src/navbar.rs)
    pub use super::navbar::Navbar;
}

mod hero;
pub use hero::Hero;

mod echo;
pub use echo::Echo;

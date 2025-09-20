//! Shared UI crate for Looplace. Most cross-platform logic and views live here.

pub mod core;
pub mod results;
pub mod tasks;
pub mod views;

mod navbar;
pub mod components {
    pub use super::navbar::Navbar;
}

mod hero;
pub use hero::Hero;

mod echo;
pub use echo::Echo;

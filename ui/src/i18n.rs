//! Internationalization (i18n) support for `looplace-ui`.
//!
//! This module wires together:
//! - `i18n-embed` (language selection + asset loading)
//! - `fluent` (message formatting)
//! - `rust-embed` (compile-time embedding of `.ftl` files)
//! - `i18n-embed-fl` (`fl!` macro for compile‑time checked lookups)
//!
//! Folder layout (relative to this crate root):
//! ```text
//! i18n.toml
//! i18n/
//!   en-US/looplace-ui.ftl   (fallback/reference)
//!   es-ES/looplace-ui.ftl   (additional locale)
//!   fr-FR/looplace-ui.ftl   (additional locale)
//! ```
//!
//! Usage in a component (after calling `i18n::init()` once at app start):
//! ```ignore
//! use crate::i18n::init;
//! use crate::t;
//! init(); // idempotent
//! let home_label = t!("nav-home");
//! ```
//!
//! To add a new locale:
//! 1. Copy `en-US/looplace-ui.ftl` to `i18n/<lang-id>/looplace-ui.ftl`.
//! 2. Translate each message value (keep IDs and variable placeholders identical).
//! 3. Run tests to ensure completeness.
//!
//! Platform notes:
//! - Desktop: uses `DesktopLanguageRequester` (OS locale list).
//! - Web/WASM: uses `WebLanguageRequester` (`navigator.languages`).
//! - Assets are always embedded on WASM (we enable `debug-embed` feature in that target-specific dependency section).
//!
//! Public API surface:
//! - `init()` – load localization bundles (safe to call multiple times).
//! - `set_language(tag: &str)` – switch language at runtime.
//! - `available_languages()` – discover embedded language tags (for a picker).
//! - Helper fns: `tr_nav_*`, `tr_tagline()` etc. (ergonomic lookup wrappers).
//! - `fl` macro re-export (for direct keyed access when needed).
//! - `LOADER` – global `FluentLanguageLoader` consumed by helpers & `fl!` macro.
//!
//! NOTE: The hyphenated filename `looplace-ui.ftl` is canonical across all locales.
use std::sync::Once;

use i18n_embed::fluent::FluentLanguageLoader;
use once_cell::sync::Lazy;
use rust_embed::Embed;
use unic_langid::LanguageIdentifier;

pub use i18n_embed_fl::fl; // Re-export for convenience.

/// Ergonomic translation macro.
/// Examples:
///     t!("nav-home")
///     t!("hello-user", name = "Emma")
///
/// This expands to `fl!(&*LOADER, ...)` keeping callsites short while
/// ensuring all lookups route through the shared loader.
#[macro_export]
macro_rules! t {
    ($key:literal) => {
        $crate::i18n::fl!(&*$crate::i18n::LOADER, $key)
    };
    ($key:literal, $( $arg:ident = $value:expr ),+ $(,)?) => {
        $crate::i18n::fl!(&*$crate::i18n::LOADER, $key, $( $arg = $value ),+ )
    };
}

/// Fluent "domain" (matches the crate / the fallback FTL filename).
///
/// Fallback file path must be: `i18n/en-US/{DOMAIN}.ftl`
const DOMAIN: &str = "looplace-ui"; // pinned explicitly (avoid relying on env! during macro domain resolution)

/// Embed all locale folders under `i18n/`.
#[derive(Embed)]
#[folder = "i18n"]
struct Localizations;

/// Global language loader used with the `fl!` macro.
pub static LOADER: Lazy<FluentLanguageLoader> = Lazy::new(|| {
    let fallback: LanguageIdentifier = "en-US".parse().expect("valid fallback language identifier");
    FluentLanguageLoader::new(DOMAIN, fallback)
});

static INIT: Once = Once::new();

/// Initialize i18n (idempotent).
pub fn init() {
    INIT.call_once(|| {
        let requested = requested_languages();
        if let Err(err) = i18n_embed::select(&*LOADER, &Localizations, &requested) {
            eprintln!("[i18n] Failed selecting languages ({err}); continuing with fallback");
        }
    });
}

/// Switch language at runtime. If `tag` cannot be parsed it is ignored (Ok returned).
pub fn set_language(tag: &str) -> Result<(), i18n_embed::I18nEmbedError> {
    let lang: LanguageIdentifier = match tag.parse() {
        Ok(l) => l,
        Err(_) => return Ok(()), // Silently ignore invalid tags.
    };
    i18n_embed::select(&*LOADER, &Localizations, &[lang]).map(|_| ())
}

/// List available (embedded) language identifiers.
pub fn available_languages() -> Vec<String> {
    let mut langs = Localizations::iter()
        .filter_map(|path| path.split('/').next().map(|s| s.to_string()))
        .collect::<Vec<_>>();
    langs.sort();
    langs.dedup();
    langs
}

#[cfg(target_arch = "wasm32")]
fn requested_languages() -> Vec<LanguageIdentifier> {
    i18n_embed::WebLanguageRequester::requested_languages()
}

#[cfg(not(target_arch = "wasm32"))]
fn requested_languages() -> Vec<LanguageIdentifier> {
    i18n_embed::DesktopLanguageRequester::requested_languages()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::fl;

    #[test]
    fn fallback_language_is_present() {
        assert!(available_languages().iter().any(|l| l == "en-US"));
    }

    #[test]
    fn basic_lookup_works() {
        init();
        let s = fl!(&*LOADER, "nav-home");
        assert_eq!(s, "Home");
    }

    #[test]
    fn dynamic_language_switch_reverts_on_failure() {
        init();
        let before = fl!(&*LOADER, "nav-home");
        let _ = set_language("zz-ZZ");
        let after = fl!(&*LOADER, "nav-home");
        assert_eq!(before, after);
    }
}

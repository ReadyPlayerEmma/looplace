#![cfg(test)]
/*!
Theme selector lint for the desktop build.

Purpose:
- Ensure that critical CSS selectors required by the desktop UI (especially the Results
  experience and readiness advisories) remain present in the unified shared theme:
  ui/assets/theme/main.css
- Fail fast if a refactor accidentally drops or renames core classes, preventing a
  silent styling regression in packaged (embedded) desktop builds.

How it works:
- We compile‑time embed the unified theme using `include_str!` pointing to the shared
  `ui/` location (mirrors the constant in `desktop/src/main.rs`).
- We assert presence of a curated set of selectors / tokens.
- If you intentionally rename or remove a selector:
    1. Update the React/Dioxus component markup.
    2. Adjust this test's REQUIRED_SELECTORS accordingly.

Why not parse CSS properly?
- A lightweight substring presence check is sufficient as an early warning.
- Keeping zero extra dependencies avoids increasing compile times.

Extending:
- Add new selectors to REQUIRED_SELECTORS when introducing structural CSS relied
  upon by Rust components (especially for charts, results lists, readiness banners, etc).
*/

const THEME_CSS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../ui/assets/theme/main.css"
));

/// Core selectors / tokens that must exist in the shared theme for desktop.
const REQUIRED_SELECTORS: &[&str] = &[
    // Global / layout
    ":root",
    "body {",
    ".page {",
    // Buttons & shared UI
    ".button {",
    ".button--primary",
    ".button--accent",
    ".button--ghost",
    // Task readiness advisory
    ".task-readiness",
    ".task-readiness--early",
    ".task-readiness--ready",
    // Results container & cards
    ".results__header",
    ".results__panels",
    ".results-card",
    ".results-card__header",
    ".results-card__meta",
    ".results-card__placeholder",
    // Results list
    ".results-list__items",
    ".results-list__item",
    ".results-list__item--active",
    ".results-list__button",
    ".results-list__metric",
    ".results-list__metric-label",
    ".results-list__metric-value",
    // Results detail
    ".results-detail__summary",
    ".results-detail__grid",
    ".results-detail__metric-label",
    ".results-detail__qc",
    // Highlights & charts
    ".results-highlights",
    ".results-chart",
    ".results-chart__title",
    ".results-chart__legend-swatch--lapses",
    ".results-chart__legend-swatch--false",
    ".results-highlight",
    ".results-highlight__value",
    // Export panel
    ".results-export__summary",
    ".results-export__actions",
    // Media query token (sanity check responsive block exists)
    "@media (max-width: 720px)",
];

#[test]
fn unified_theme_contains_required_selectors() {
    let mut missing = Vec::new();
    for sel in REQUIRED_SELECTORS {
        if !THEME_CSS.contains(sel) {
            missing.push(*sel);
        }
    }

    if !missing.is_empty() {
        panic!(
            "Missing {} required CSS selectors/tokens in unified theme:\n{}",
            missing.len(),
            missing.join("\n")
        );
    }
}

#[test]
fn unified_theme_not_trivially_empty() {
    let non_ws_len = THEME_CSS.chars().filter(|c| !c.is_whitespace()).count();
    assert!(
        non_ws_len > 4_000,
        "Embedded theme appears unexpectedly small ({} non-whitespace chars) – \
         did the file get truncated or path change?",
        non_ws_len
    );
}

#[test]
fn readiness_block_consistency() {
    // Ensure readiness status classes have expected pairing.
    let has_status = THEME_CSS.contains(".task-readiness__status");
    let has_detail = THEME_CSS.contains(".task-readiness__detail");
    assert!(
        has_status && has_detail,
        "Readiness advisory sub‑selectors missing (status: {has_status}, detail: {has_detail})"
    );
}

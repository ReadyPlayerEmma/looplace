#![cfg(test)]
//! Ensures the embedded desktop CSS (`assets/main.css`) remains present & non‑trivial.
//!
//! Rationale:
//! - We rely on `include_str!` at compile time to inline styles (no external file in Windows zip).
//! - An accidental deletion or empty file would silently degrade styling only at *runtime*.
//! - This test fails the build early if the file goes missing or is blank.
//!
//! If you intentionally rename or relocate the CSS file, update both this test and the
//! `include_str!` in `desktop/src/main.rs`.

const EMBEDDED_CSS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/main.css"));

#[test]
fn embedded_css_file_exists_and_is_not_empty() {
    assert!(
        !EMBEDDED_CSS.trim().is_empty(),
        "Embedded CSS file appears to be empty. If this is intentional, remove the test."
    );
}

#[test]
fn embedded_css_contains_expected_tokens() {
    // Quick sanity tokens that should exist in our theme.
    let required = [
        "--color-bg",
        ".task-readiness",
        "body {",
        ".button--primary",
    ];
    for token in required {
        assert!(
            EMBEDDED_CSS.contains(token),
            "Expected token `{token}` missing from embedded CSS"
        );
    }
}

use std::collections::{BTreeSet, HashSet};

/// Translation completeness test.
/// Ensures every non‑fallback locale provides *at least* the keys present
/// in the fallback (en-US) `looplace-ui.ftl`.
///
/// This is a lightweight parser:
/// - Ignores comment lines starting with `#`
/// - Treats any line of the form `key =` or `key=` as a message definition
/// - Skips blank / attribute / continuation lines
/// - Does not attempt to parse multi-line pattern bodies (only keys)
///
/// If you add a new locale:
/// 1. Create `ui/i18n/<locale>/looplace-ui.ftl`
/// 2. Copy all keys from `en-US/looplace-ui.ftl`
/// 3. Run `cargo test -p looplace-ui` to confirm completeness.
#[test]
fn all_locales_have_all_fallback_keys() {
    // Embed the FTL sources at compile time.
    // (If you add a new locale, register it here.)
    const EN_US: &str = include_str!("../i18n/en-US/looplace-ui.ftl");
    const ES_ES: &str = include_str!("../i18n/es-ES/looplace-ui.ftl");
    const FR_FR: &str = include_str!("../i18n/fr-FR/looplace-ui.ftl");

    let fallback_keys = extract_keys(EN_US);

    // Ensure fallback itself has no duplicates and at least one key.
    assert!(
        !fallback_keys.is_empty(),
        "Fallback (en-US) contains no keys."
    );
    assert_no_dup_keys(EN_US, "en-US");

    let locales: &[(&str, &str)] = &[
        ("es-ES", ES_ES),
        ("fr-FR", FR_FR),
        // Add new locales here.
    ];

    let mut failures = Vec::new();

    for (locale, src) in locales {
        assert_no_dup_keys(src, locale);

        let keys = extract_keys(src);
        let mut missing: BTreeSet<String> = BTreeSet::new();

        for k in &fallback_keys {
            if !keys.contains(k) {
                missing.insert(k.clone());
            }
        }

        if !missing.is_empty() {
            failures.push(format!(
                "Locale {locale} is missing {} key(s):\n  {}",
                missing.len(),
                missing.into_iter().collect::<Vec<_>>().join("\n  ")
            ));
        }
    }

    if !failures.is_empty() {
        panic!(
            "Translation completeness check failed:\n\n{}\n\nHint: copy the missing keys from en-US, then translate.",
            failures.join("\n\n")
        );
    }
}

/// Extract message keys from a Fluent file (simple heuristic).
fn extract_keys(src: &str) -> HashSet<String> {
    let mut keys = HashSet::new();

    for line in src.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Skip attribute or continuation lines (start with '.' or indent).
        if line.starts_with('.') {
            continue;
        }
        // Basic pattern: key [space]* '='
        if let Some(eq_pos) = line.find('=') {
            let (left, _right) = line.split_at(eq_pos);
            let key = left.trim();
            if !key.is_empty()
                && !key.contains(' ')
                && !key.contains('\t')
                && !key.starts_with('[')
                && !key.starts_with('@')
            {
                keys.insert(key.to_string());
            }
        }
    }

    keys
}

/// Assert no duplicate key definitions in a single FTL file (rudimentary).
fn assert_no_dup_keys(src: &str, locale: &str) {
    let mut seen = HashSet::new();
    let mut dups = BTreeSet::new();

    for line in src.lines() {
        let raw = line;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('.') {
            continue;
        }
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim();
            if !key.is_empty()
                && !key.contains(' ')
                && !key.contains('\t')
                && !key.starts_with('[')
                && !key.starts_with('@')
            {
                if !seen.insert(key.to_string()) {
                    dups.insert(format!("{key}  (line: \"{raw}\")"));
                }
            }
        }
    }

    if !dups.is_empty() {
        panic!(
            "Duplicate key definitions in {locale}:\n  {}",
            dups.into_iter().collect::<Vec<_>>().join("\n  ")
        );
    }
}

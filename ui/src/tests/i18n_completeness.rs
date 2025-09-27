use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Name of the canonical FTL file per locale.
const FTL_FILENAME: &str = "looplace-ui.ftl";

/// Root (relative to crate) for i18n assets.
const I18N_DIR: &str = "i18n";

/// Simple parser: extract message IDs from a Fluent file.
/// We treat any line that starts (after optional whitespace) with:
///    <identifier> =
/// as a message definition. Comments, terms (-prefix), blank lines ignored.
fn parse_ftl_keys(content: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Skip Fluent "terms" (which start with a dash) for now - only messages are being used.
        if line.starts_with('-') {
            continue;
        }
        // Find pattern: id = ...
        if let Some(eq_pos) = line.find('=') {
            let (maybe_id, _) = line.split_at(eq_pos);
            let id = maybe_id.trim();
            if id.chars().all(valid_key_char) && !id.is_empty() {
                keys.insert(id.to_string());
            }
        }
    }
    keys
}

fn valid_key_char(c: char) -> bool {
    matches!(c, 'a'..='z' | '0'..='9' | '-' )
}

/// Extract all t!("...") occurrences (and t!("...", ...) forms) from source files under `src/`.
/// This is intentionally conservative: it only matches a direct literal first argument.
///
/// NOTE: This will not catch:
///   - fl! macro usage directly
///   - dynamically constructed IDs
///   - macro indirection
///
/// That's acceptable for the completeness guard (we rely on direct usage patterns).
fn extract_translation_keys_from_source(src_root: &Path) -> HashSet<String> {
    let mut found = HashSet::new();
    let mut stack = vec![src_root.to_path_buf()];

    while let Some(path) = stack.pop() {
        if path.is_dir() {
            if let Ok(read_dir) = fs::read_dir(&path) {
                for entry in read_dir.flatten() {
                    let p = entry.path();
                    // Skip target directories inside src/tests (none expected) and generated code if any.
                    if p.file_name().and_then(|s| s.to_str()) == Some("target") {
                        continue;
                    }
                    stack.push(p);
                }
            }
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }

        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };

        let bytes = content.as_bytes();
        let needle = b"t!(\"";
        let mut i = 0;
        while let Some(pos) = content[i..]
            .as_bytes()
            .windows(needle.len())
            .position(|w| w == needle)
        {
            let start = i + pos + needle.len();
            // Scan until next unescaped quote
            let mut j = start;
            while j < bytes.len() {
                let b = bytes[j];
                if b == b'\\' {
                    j += 2;
                    continue;
                }
                if b == b'"' {
                    // Extract
                    if let Ok(key) = std::str::from_utf8(&bytes[start..j]) {
                        if key.chars().all(valid_key_char) {
                            found.insert(key.to_string());
                        }
                    }
                    break;
                }
                j += 1;
            }
            i = j + 1;
        }
    }

    found
}

fn collect_locale_dirs(i18n_root: &Path) -> Vec<String> {
    let mut dirs = Vec::new();
    if let Ok(read_dir) = fs::read_dir(i18n_root) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                    // Basic heuristic: locale folders contain at least one hyphen or are en-US style.
                    if name.contains('-') {
                        dirs.push(name.to_string());
                    }
                }
            }
        }
    }
    dirs.sort();
    dirs
}

#[test]
fn i18n_completeness() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let i18n_root = crate_root.join(I18N_DIR);

    // 1. Fallback locale (en-US) must exist
    let fallback_dir = i18n_root.join("en-US");
    assert!(
        fallback_dir.exists(),
        "Missing fallback locale directory: {:?}",
        fallback_dir
    );

    let fallback_file = fallback_dir.join(FTL_FILENAME);
    let fallback_content =
        fs::read_to_string(&fallback_file).expect("Failed to read fallback FTL file");
    let fallback_keys = parse_ftl_keys(&fallback_content);
    assert!(
        !fallback_keys.is_empty(),
        "No message keys parsed from fallback FTL: {:?}",
        fallback_file
    );

    // 2. Gather all referenced keys in Rust sources.
    let src_root = crate_root.join("src");
    let referenced_keys = extract_translation_keys_from_source(&src_root);

    // 3. Report any referenced keys missing in fallback.
    let mut missing_in_fallback: Vec<_> = referenced_keys
        .iter()
        .filter(|k| !fallback_keys.contains(*k))
        .collect();
    missing_in_fallback.sort();

    if !missing_in_fallback.is_empty() {
        panic!(
            "Referenced translation keys missing in fallback ({}):\n{}",
            missing_in_fallback.len(),
            missing_in_fallback.join("\n")
        );
    }

    // 4. For each locale, ensure no key is missing relative to fallback.
    let locales = collect_locale_dirs(&i18n_root);
    let mut per_locale_missing: HashMap<String, Vec<String>> = HashMap::new();

    for locale in locales {
        let path = i18n_root.join(&locale).join(FTL_FILENAME);
        if !path.exists() {
            panic!(
                "Locale folder {:?} missing expected file {:?}",
                locale, path
            );
        }
        let content = fs::read_to_string(&path).unwrap_or_default();
        let keys = parse_ftl_keys(&content);

        let mut missing: Vec<_> = fallback_keys
            .iter()
            .filter(|k| !keys.contains(*k))
            .cloned()
            .collect();
        if !missing.is_empty() {
            missing.sort();
            per_locale_missing.insert(locale, missing);
        }
    }

    if !per_locale_missing.is_empty() {
        let mut report = String::from("Locales with missing translations relative to fallback:\n");
        for (loc, miss) in per_locale_missing.iter() {
            report.push_str(&format!("  {loc} ({} missing)\n", miss.len()));
            for k in miss {
                report.push_str(&format!("    {k}\n"));
            }
        }
        panic!("{report}");
    }

    // 5. (Optional) Warn unused fallback keys: not a failure, but helpful.
    let unused: Vec<_> = fallback_keys
        .iter()
        .filter(|k| !referenced_keys.contains(*k))
        .collect();
    // We only print this; avoiding failure to allow staged rollout.
    if !unused.is_empty() {
        eprintln!(
            "[i18n] NOTE: {} fallback keys unused in Rust sources (first 20 shown):\n{}",
            unused.len(),
            unused
                .iter()
                .take(20)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}

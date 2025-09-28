#!/usr/bin/env bash
#
# macOS Bundle Script (CI-first)
# ------------------------------
# Single source of truth for producing the Looplace macOS .app bundle & zip artifact.
#
# Design goals:
#   * CI-first: Reproduces the exact layout previously implemented inline in GitHub workflows.
#   * Deterministic: Given the same inputs, produces byte-identical folder/zip structure (aside from
#     filesystem metadata inherent to codesigning & compression timestamps).
#   * Parameterizable via environment variables (no required flags for CI use).
#   * Local-friendly: Can be run manually for smoke tests with minimal env setup.
#   * Verifiable: Optional STRICT mode validates expected output structure.
#
# Resulting artifact layout (matches legacy inline GH logic):
#
#   dist/
#     <OUT_BASENAME>/Looplace.app/              # Unzipped convenience copy (post-packaging)
#     <OUT_BASENAME>.zip                        # Zip whose top-level folder is <OUT_BASENAME>-macos/Looplace.app
#
# Inside the zip:
#   <OUT_BASENAME>-macos/Looplace.app/Contents/{Info.plist,MacOS/Looplace,Resources}
#
# Environment Variables (tunable):
#   APP_NAME        Display + executable name inside the bundle (default: Looplace)
#   PROFILE         Cargo profile used for the binary path lookup (default: release)
#   RUST_TARGET     Target triple (e.g. aarch64-apple-darwin). Strongly recommended in CI.
#   OUT_BASENAME    Basename for output artifacts. Examples:
#                     build workflow: looplace-desktop-aarch64-apple-darwin
#                     release workflow: looplace-desktop-v0.1.7-aarch64-apple-darwin
#                   If unset, defaults to: looplace-desktop${RUST_TARGET:+-$RUST_TARGET}
#   OUTPUT_DIR      Root directory for produced artifacts (default: dist)
#   SIGN_IDENTITY   codesign identity (default: - for ad-hoc)
#   STRICT          If "1", perform structure validation (default: 0)
#   VERBOSE         If "1", increase logging (default: 0)
#
# Exit Codes:
#   0 on success; non-zero on failure with a descriptive message.
#
# Example (CI build job):
#   RUST_TARGET=aarch64-apple-darwin \
#   OUT_BASENAME="looplace-desktop-aarch64-apple-darwin" \
#   ./scripts/macos/bundle.sh
#
# Example (CI release job):
#   RUST_TARGET=aarch64-apple-darwin \
#   OUT_BASENAME="looplace-desktop-v0.1.7-aarch64-apple-darwin" \
#   ./scripts/macos/bundle.sh
#
# Example (local quick run after building):
#   RUST_TARGET=$(rustc -vV | grep host | awk '{print $2}') \
#   ./scripts/macos/bundle.sh
#
set -euo pipefail

########################################
# Logging helpers
########################################
_color() { local c="$1"; shift; printf "\033[%sm%s\033[0m" "$c" "$*"; }
info()   { printf "🔧 %s\n" "$*"; }
warn()   { printf "⚠️  %s\n" "$*" >&2; }
err()    { printf "❌ %s\n" "$*" >&2; }
ok()     { printf "✅ %s\n" "$*"; }
debug()  { [[ "${VERBOSE:-0}" == "1" ]] && printf "🛈 %s\n" "$*" || true; }

########################################
# Usage (only if explicitly requested)
########################################
if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  sed -n '1,/^set -euo pipefail/p' "$0"
  exit 0
fi

########################################
# Inputs / Defaults
########################################
APP_NAME="${APP_NAME:-Looplace}"
PROFILE="${PROFILE:-release}"
RUST_TARGET="${RUST_TARGET:-}"
OUTPUT_DIR="${OUTPUT_DIR:-dist}"
SIGN_IDENTITY="${SIGN_IDENTITY:--}"
STRICT="${STRICT:-0}"
VERBOSE="${VERBOSE:-0}"
BIN_NAME="looplace-desktop"

# Derive OUT_BASENAME if not provided
if [[ -z "${OUT_BASENAME:-}" ]]; then
  if [[ -n "${RUST_TARGET}" ]]; then
    OUT_BASENAME="looplace-desktop-${RUST_TARGET}"
  else
    OUT_BASENAME="looplace-desktop"
    warn "OUT_BASENAME not set and RUST_TARGET empty; using '${OUT_BASENAME}' (non-deterministic across targets)."
  fi
fi

########################################
# Resolve binary path
########################################
resolve_binary() {
  local candidate=""
  if [[ -n "${RUST_TARGET}" ]]; then
    candidate="target/${RUST_TARGET}/${PROFILE}/${BIN_NAME}"
    if [[ -f "${candidate}" ]]; then
      echo "${candidate}"
      return 0
    fi
  fi
  # Fallback search
  candidate="$(find target -type f -path "*/${PROFILE}/${BIN_NAME}" -maxdepth 5 2>/dev/null | head -n1 || true)"
  if [[ -n "${candidate}" && -f "${candidate}" ]]; then
    echo "${candidate}"
    return 0
  fi
  return 1
}

BIN_PATH="$(resolve_binary || true)"
if [[ -z "${BIN_PATH}" ]]; then
  err "Desktop binary not found. Build first, e.g.: cargo build --release -p looplace-desktop${RUST_TARGET:+ --target $RUST_TARGET}"
  exit 1
fi
debug "Resolved binary path: ${BIN_PATH}"

########################################
# Derive version from desktop Cargo.toml
########################################
if [[ ! -f desktop/Cargo.toml ]]; then
  err "Missing desktop/Cargo.toml (required for version extraction)."
  exit 1
fi
VERSION="$(grep -m1 '^version' desktop/Cargo.toml | sed 's/.*"\(.*\)"/\1/')"
if [[ -z "${VERSION}" ]]; then
  err "Failed to extract version from desktop/Cargo.toml"
  exit 1
fi
debug "Extracted version: ${VERSION}"

########################################
# Prepare paths mimicking CI layout
########################################
# Uncompressed convenience app path:
BUNDLE_ROOT="${OUTPUT_DIR}/${OUT_BASENAME}/${APP_NAME}.app"
MACOS_DIR="${BUNDLE_ROOT}/Contents/MacOS"
RESOURCES_DIR="${BUNDLE_ROOT}/Contents/Resources"

# Zip path (top-level folder inside zip will be "${OUT_BASENAME}-macos")
ZIP_PATH="${OUTPUT_DIR}/${OUT_BASENAME}.zip"
ZIP_PARENT_NAME="${OUT_BASENAME}-macos"
ZIP_PARENT_DIR="${OUTPUT_DIR}/${ZIP_PARENT_NAME}"

########################################
# Create bundle structure
########################################
info "Creating macOS bundle structure"
rm -rf "${BUNDLE_ROOT}" "${ZIP_PARENT_DIR}"
mkdir -p "${MACOS_DIR}" "${RESOURCES_DIR}"

cp "${BIN_PATH}" "${MACOS_DIR}/${APP_NAME}"
chmod +x "${MACOS_DIR}/${APP_NAME}"

# Copy Info.plist template
INFO_PLIST_TEMPLATE="desktop/macos/Info.plist"
if [[ ! -f "${INFO_PLIST_TEMPLATE}" ]]; then
  err "Info.plist template missing at ${INFO_PLIST_TEMPLATE}"
  exit 1
fi
cp "${INFO_PLIST_TEMPLATE}" "${BUNDLE_ROOT}/Contents/Info.plist"

# Copy assets if present
if [[ -d desktop/assets ]]; then
  debug "Copying assets (stripping extended attributes)"
  # -R recurse; -X skip extended attributes to avoid AppleDouble (._*) files later
  cp -R -X desktop/assets "${MACOS_DIR}/assets"
  # As an extra safeguard, delete any pre-existing AppleDouble or .DS_Store remnants
  find "${MACOS_DIR}/assets" -name '._*' -type f -delete 2>/dev/null || true
  find "${MACOS_DIR}/assets" -name '.DS_Store' -type f -delete 2>/dev/null || true
fi

########################################
# Stamp version into Info.plist
########################################
PLIST="${BUNDLE_ROOT}/Contents/Info.plist"
if command -v /usr/libexec/PlistBuddy >/dev/null 2>&1; then
  /usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString ${VERSION}" "${PLIST}" || true
  /usr/libexec/PlistBuddy -c "Set :CFBundleVersion ${VERSION}" "${PLIST}" || true
else
  warn "PlistBuddy not found; version fields not stamped (expected in CI)."
fi

########################################
# Normalize extended attributes & codesign
########################################
if command -v xattr >/dev/null 2>&1; then
  # Some macOS environments (notably newer CLI tools) do not support a recursive -r flag with xattr.
  # Attempt a direct clear on the bundle root; if any extended attributes remain deeper, fall back to find.
  if ! xattr -c "${BUNDLE_ROOT}" 2>/dev/null; then
    # Fallback: clear recursively file-by-file (best-effort; ignore failures).
    find "${BUNDLE_ROOT}" -exec xattr -c {} \; 2>/dev/null || true
  fi
fi
if command -v codesign >/dev/null 2>&1; then
  # Ad-hoc sign (or provided identity). Fail softly to avoid pipeline hard-stop if codesign quirks appear.
  codesign --force --deep --sign "${SIGN_IDENTITY}" "${BUNDLE_ROOT}" || warn "codesign returned non-zero (continuing)"
else
  warn "codesign not found; bundle will be unsigned."
fi

########################################
# Package zip replicating legacy CI pattern
########################################
info "Packaging zip artifact"
mkdir -p "${ZIP_PARENT_DIR}"
mv "${BUNDLE_ROOT}" "${ZIP_PARENT_DIR}/"

# Remove AppleDouble resource fork files and macOS metadata before zipping to
# match legacy CI artifacts (which lacked these entries).
find "${ZIP_PARENT_DIR}" -name '._*' -type f -delete 2>/dev/null || true
find "${ZIP_PARENT_DIR}" -name '.DS_Store' -type f -delete 2>/dev/null || true

mkdir -p "${OUTPUT_DIR}"
(
  cd "${OUTPUT_DIR}"
  rm -f "${ZIP_PATH##${OUTPUT_DIR}/}"
  # Keep parent to retain <OUT_BASENAME>-macos folder level
  # Add --norsrc to suppress AppleDouble (._*) and extended attribute forks so
  # they do not appear in the zip manifest (parity with legacy CI artifacts).
  ditto -c -k --norsrc --keepParent "${ZIP_PARENT_NAME}" "${ZIP_PATH##${OUTPUT_DIR}/}"
)

# Restore uncompressed app copy to expected path
mkdir -p "$(dirname "${BUNDLE_ROOT}")"
mv "${ZIP_PARENT_DIR}/${APP_NAME}.app" "${BUNDLE_ROOT}"
rmdir "${ZIP_PARENT_DIR}" || true

########################################
# STRICT validation (structure assertions)
########################################
if [[ "${STRICT}" == "1" ]]; then
  info "Running STRICT validation"
  declare -a MUST_EXIST=(
    "${BUNDLE_ROOT}/Contents/MacOS/${APP_NAME}"
    "${BUNDLE_ROOT}/Contents/Info.plist"
    "${ZIP_PATH}"
  )
  for path in "${MUST_EXIST[@]}"; do
    [[ -e "${path}" ]] || { err "Missing expected artifact: ${path}"; exit 1; }
  done

  # Check internal zip top-level layout (use unzip -Z1 for raw file names to avoid column formatting)
  if command -v unzip >/dev/null 2>&1; then
    ZIP_LISTING="$(mktemp -t looplace_zip_listing.XXXXXX)"
    unzip -Z1 "${ZIP_PATH}" > "${ZIP_LISTING}" 2>/dev/null || {
      err "Unable to list zip contents for STRICT validation"
      cat "${ZIP_LISTING}" || true
      rm -f "${ZIP_LISTING}"
      exit 1
    }
    REQUIRED_ENTRY="${ZIP_PARENT_NAME}/Looplace.app/Contents/MacOS/${APP_NAME}"
    if ! grep -F -q "${REQUIRED_ENTRY}" "${ZIP_LISTING}"; then
      echo "---- ZIP CONTENTS (first 100 lines) ----" >&2
      head -n 100 "${ZIP_LISTING}" >&2 || true
      echo "---- END ZIP CONTENTS ----" >&2
      err "Zip structure mismatch: expected entry '${REQUIRED_ENTRY}'"
      rm -f "${ZIP_LISTING}"
      exit 1
    fi
    rm -f "${ZIP_LISTING}"
  else
    warn "unzip not available; skipping internal zip structure check."
  fi
  ok "STRICT validation passed."
fi

########################################
# Output summary
########################################
ok "Bundle created"
echo "  Version:        ${VERSION}"
echo "  App (unzipped): ${BUNDLE_ROOT}"
echo "  Zip artifact:   ${ZIP_PATH}"

# Emit GitHub Actions outputs if possible (compatible with new $GITHUB_OUTPUT contract)
if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
  {
    echo "zip_path=${ZIP_PATH}"
    echo "app_path=${BUNDLE_ROOT}"
    echo "version=${VERSION}"
  } >> "${GITHUB_OUTPUT}"
fi

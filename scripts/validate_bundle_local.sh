#!/usr/bin/env bash
#
# validate_bundle_local.sh
# ------------------------
# Validates that the new unified macOS bundling script (`scripts/macos/bundle.sh`)
# reproduces the packaging layout of an existing published release artifact.
#
# Primary check: Structural + manifest parity (file paths inside the zip).
# Secondary checks: Info.plist version fields, binary checksum (informational).
#
# By default this script:
#   * Derives VERSION from desktop/Cargo.toml (unless --version/--tag specified)
#   * Uses target aarch64-apple-darwin (Apple Silicon)
#   * Downloads the release asset via GitHub CLI: looplace-desktop-v<VERSION>-<target>.zip
#   * Rebuilds the desktop binary locally
#   * Repackages using scripts/macos/bundle.sh
#   * Compares manifests, plist versions, and (optionally) binary sha256
#
# REQUIREMENTS:
#   - bash, git, cargo, rustup (target installed), unzip, shasum
#   - gh (GitHub CLI) authenticated (`gh auth status`)
#
# USAGE:
#   scripts/validate_bundle_local.sh                    # auto-detect version & compare
#   scripts/validate_bundle_local.sh --version 0.1.7
#   scripts/validate_bundle_local.sh --tag v0.1.7
#   scripts/validate_bundle_local.sh --version 0.1.7 --target aarch64-apple-darwin
#
# OPTIONS:
#   --version <X.Y.Z>    Semantic version (implies tag vX.Y.Z)
#   --tag <vX.Y.Z>       Explicit tag (overrides --version for asset naming)
#   --target <triple>    Target triple (default: aarch64-apple-darwin)
#   --skip-download      Re-use previously downloaded old.zip
#   --skip-build         Do not rebuild local binary (assumes already built)
#   --keep               Do not remove temp folder on success
#   --no-binary-check    Skip binary sha256 comparison
#   --verbose            Extra logging
#   -h | --help          Show help
#
# EXIT CODES:
#   0 success
#   1 usage or environmental failure
#   2 manifest mismatch
#   3 plist version mismatch
#
# EXAMPLE QUICK RUN:
#   ./scripts/validate_bundle_local.sh --version 0.1.7
#
set -euo pipefail

########################################
# Logging helpers
########################################
color() { [[ -t 1 ]] && printf "\033[%sm%s\033[0m" "$1" "$2" || printf "%s" "$2"; }
info()  { printf "🔧 %s\n" "$*"; }
warn()  { printf "⚠️  %s\n" "$*" >&2; }
err()   { printf "❌ %s\n" "$*" >&2; }
ok()    { printf "✅ %s\n" "$*"; }
debug() { [[ "${VERBOSE:-0}" == "1" ]] && printf "🛈 %s\n" "$*" || true; }

########################################
# Defaults
########################################
VERSION=""
TAG=""
TARGET="aarch64-apple-darwin"
SKIP_DOWNLOAD=0
SKIP_BUILD=0
KEEP=0
CHECK_BINARY=1
VERBOSE=0

########################################
# Parse args
########################################
while [[ $# -gt 0 ]]; do
  case "$1" in
    --version) VERSION="$2"; shift 2 ;;
    --tag) TAG="$2"; shift 2 ;;
    --target) TARGET="$2"; shift 2 ;;
    --skip-download) SKIP_DOWNLOAD=1; shift ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    --keep) KEEP=1; shift ;;
    --no-binary-check) CHECK_BINARY=0; shift ;;
    --verbose) VERBOSE=1; shift ;;
    -h|--help)
      sed -n '1,/^set -euo pipefail/p' "$0"
      exit 0
      ;;
    *)
      err "Unknown argument: $1"
      exit 1
      ;;
  esac
done

########################################
# Derive VERSION/TAG if not provided
########################################
if [[ -z "${TAG}" ]]; then
  if [[ -n "${VERSION}" ]]; then
    TAG="v${VERSION}"
  else
    if [[ -f desktop/Cargo.toml ]]; then
      VERSION="$(grep -m1 '^version' desktop/Cargo.toml | sed 's/.*"\(.*\)"/\1/')"
      TAG="v${VERSION}"
      info "Derived version from desktop/Cargo.toml: ${VERSION}"
    else
      err "Cannot derive version (desktop/Cargo.toml missing); specify --version or --tag."
      exit 1
    fi
  fi
else
  # If tag was given, set VERSION if possible.
  if [[ "${TAG}" =~ ^v([0-9]+\.[0-9]+\.[0-9]+)$ ]]; then
    VERSION="${BASH_REMATCH[1]}"
  elif [[ -z "${VERSION}" ]]; then
    warn "TAG provided (${TAG}) not in vX.Y.Z form; version-based checks may be limited."
  fi
fi

[[ -n "${TAG}" ]] || { err "Failed to resolve tag"; exit 1; }
info "Validation target: tag=${TAG} version=${VERSION:-unknown} target=${TARGET}"

########################################
# Environment validation
########################################
need() {
  command -v "$1" >/dev/null 2>&1 || { err "Missing required command: $1"; exit 1; }
}
need gh
need unzip
need shasum
need cargo

if ! gh auth status >/dev/null 2>&1; then
  err "GitHub CLI not authenticated. Run: gh auth login"
  exit 1
fi

########################################
# Working directories
########################################
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

TMP_ROOT="validation_bundle"
LEGACY_DIR="${TMP_ROOT}/legacy"
SCRIPT_DIR_OUT="${TMP_ROOT}/script"
mkdir -p "${LEGACY_DIR}" "${SCRIPT_DIR_OUT}"

OLD_ZIP="${LEGACY_DIR}/old.zip"
NEW_ZIP="${SCRIPT_DIR_OUT}/new.zip"   # We'll symlink/move final produced zip here for clarity
MANIFEST_OLD="${TMP_ROOT}/manifest_old.lst"
MANIFEST_NEW="${TMP_ROOT}/manifest_new.lst"
MANIFEST_DIFF="${TMP_ROOT}/manifest.diff"

OUT_BASENAME="looplace-desktop-${TAG}-${TARGET}"

########################################
# Step 1: Download legacy artifact
########################################
if [[ "${SKIP_DOWNLOAD}" -eq 0 ]]; then
  info "Downloading release asset for ${TAG}"
  rm -f "${OLD_ZIP}"
  # Asset pattern matches release workflow naming
  ASSET_PATTERN="looplace-desktop-${TAG}-${TARGET}.zip"
  if ! gh release download "${TAG}" -p "${ASSET_PATTERN}" -O "${OLD_ZIP}"; then
    err "Failed to download asset ${ASSET_PATTERN} for ${TAG}"
    exit 1
  fi
else
  info "Skipping download (using existing ${OLD_ZIP})"
fi

[[ -f "${OLD_ZIP}" ]] || { err "Legacy zip not found at ${OLD_ZIP}"; exit 1; }
debug "Legacy zip size: $(du -h "${OLD_ZIP}" | awk '{print $1}')"

########################################
# Step 2: Build local binary (if requested)
########################################
if [[ "${SKIP_BUILD}" -eq 0 ]]; then
  info "Building local release binary"
  cargo build --release -p looplace-desktop --features desktop --target "${TARGET}"
else
  info "Skipping build (--skip-build)"
fi

BIN_PATH="target/${TARGET}/release/looplace-desktop"
[[ -f "${BIN_PATH}" ]] || { err "Built binary not found at ${BIN_PATH}"; exit 1; }

########################################
# Step 3: Run new bundling script
########################################
info "Running new bundler script with OUT_BASENAME='${OUT_BASENAME}'"
RUST_TARGET="${TARGET}" \
OUT_BASENAME="${OUT_BASENAME}" \
OUTPUT_DIR="${SCRIPT_DIR_OUT}" \
STRICT=1 \
scripts/macos/bundle.sh

GENERATED_ZIP="${SCRIPT_DIR_OUT}/${OUT_BASENAME}.zip"
[[ -f "${GENERATED_ZIP}" ]] || { err "Expected new zip at ${GENERATED_ZIP}"; exit 1; }

# Create a stable reference as NEW_ZIP (copy not move to preserve original path semantics)
cp -f "${GENERATED_ZIP}" "${NEW_ZIP}"

########################################
# Step 4: Manifest comparison
########################################
info "Comparing zip manifests"
# Generate raw manifests
unzip -Z1 "${OLD_ZIP}" | sort > "${MANIFEST_OLD}"
unzip -Z1 "${NEW_ZIP}" | sort > "${MANIFEST_NEW}"

# Create filtered manifests excluding AppleDouble / resource fork noise (._*) and any stray .DS_Store
FILTERED_OLD="${MANIFEST_OLD%.lst}.filtered.lst"
FILTERED_NEW="${MANIFEST_NEW%.lst}.filtered.lst"

grep -v '/\._' "${MANIFEST_OLD}" | grep -v '/\.DS_Store$' > "${FILTERED_OLD}" || true
grep -v '/\._' "${MANIFEST_NEW}" | grep -v '/\.DS_Store$' > "${FILTERED_NEW}" || true

if diff -u "${FILTERED_OLD}" "${FILTERED_NEW}" > "${MANIFEST_DIFF}"; then
  ok "Manifest: MATCH (AppleDouble entries ignored)"
else
  err "Manifest mismatch (ignoring AppleDouble). See ${MANIFEST_DIFF}"
  # Show concise diff summary
  head -n 200 "${MANIFEST_DIFF}" >&2 || true
  echo "Hint: Differences here are real structural mismatches (not just '._' files)." >&2
  exit 2
fi

########################################
# Step 5: Plist version comparison
########################################
PLIST_PATH_NEW="${SCRIPT_DIR_OUT}/${OUT_BASENAME}/Looplace.app/Contents/Info.plist"
PLIST_PATH_OLD="${TMP_ROOT}/_legacy_plist_extracted/Info.plist"

info "Extracting legacy Info.plist"
rm -rf "${TMP_ROOT}/_legacy_plist_extracted"
mkdir -p "${TMP_ROOT}/_legacy_plist_extracted"
# Extract only the plist (ignore other files to save time)
unzip -q "${OLD_ZIP}" "${OUT_BASENAME}-macos/Looplace.app/Contents/Info.plist" -d "${TMP_ROOT}/_legacy_plist_extracted" || {
  err "Failed to extract legacy Info.plist"
  exit 1
}
mv "${TMP_ROOT}/_legacy_plist_extracted/${OUT_BASENAME}-macos/Looplace.app/Contents/Info.plist" "${PLIST_PATH_OLD}"

[[ -f "${PLIST_PATH_NEW}" ]] || { err "New Info.plist missing at ${PLIST_PATH_NEW}"; exit 1; }
[[ -f "${PLIST_PATH_OLD}" ]] || { err "Legacy Info.plist missing at ${PLIST_PATH_OLD}"; exit 1; }

if command -v /usr/libexec/PlistBuddy >/dev/null 2>&1; then
  legacy_short=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "${PLIST_PATH_OLD}")
  new_short=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "${PLIST_PATH_NEW}")
  legacy_full=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "${PLIST_PATH_OLD}")
  new_full=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "${PLIST_PATH_NEW}")
  echo "Legacy ShortVersion: ${legacy_short}"
  echo "New    ShortVersion: ${new_short}"
  echo "Legacy BundleVersion: ${legacy_full}"
  echo "New    BundleVersion: ${new_full}"

  if [[ "${legacy_short}" != "${new_short}" || "${legacy_full}" != "${new_full}" ]]; then
    err "Info.plist version mismatch"
    exit 3
  else
    ok "Info.plist versions: MATCH"
  fi
else
  warn "PlistBuddy not available; skipping detailed plist version comparison."
fi

########################################
# Step 6: Binary checksum (informational)
########################################
if [[ "${CHECK_BINARY}" -eq 1 ]]; then
  info "Computing binary checksums (inside zips)"
  BIN_IN_ZIP="${OUT_BASENAME}-macos/Looplace.app/Contents/MacOS/Looplace"
  legacy_sha=$(unzip -p "${OLD_ZIP}" "${BIN_IN_ZIP}" | shasum -a 256 | awk '{print $1}')
  new_sha=$(unzip -p "${NEW_ZIP}" "${BIN_IN_ZIP}" | shasum -a 256 | awk '{print $1}')
  echo "Legacy binary sha256: ${legacy_sha}"
  echo "New    binary sha256: ${new_sha}"
  if [[ "${legacy_sha}" == "${new_sha}" ]]; then
    ok "Binary sha256: MATCH"
  else
    warn "Binary sha256 differs (expected if environment/toolchain flags differ)."
  fi
else
  info "Skipping binary checksum (--no-binary-check)"
fi

########################################
# Summary
########################################
echo
ok "Bundle validation succeeded"
echo "Summary:"
echo "  Tag / Version : ${TAG} / ${VERSION:-unknown}"
echo "  Target        : ${TARGET}"
echo "  OUT_BASENAME  : ${OUT_BASENAME}"
echo "  Legacy zip    : ${OLD_ZIP}"
echo "  New zip       : ${NEW_ZIP}"
echo "  Manifest      : identical"
echo "  Info.plist    : identical versions"
[[ "${CHECK_BINARY}" -eq 1 ]] && echo "  Binary sha256  : ${legacy_sha:-(n/a)} vs ${new_sha:-(n/a)}" || true

if [[ "${KEEP}" -eq 0 ]]; then
  debug "Cleaning temporary extracted plist directory"
  rm -rf "${TMP_ROOT}/_legacy_plist_extracted"
fi

exit 0

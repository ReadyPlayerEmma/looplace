#!/usr/bin/env bash
#
# Looplace Release Helper (CI-complementary)
# -----------------------------------------
# Purpose:
#   Automate ONLY the pre-tag local responsibilities that the GitHub Actions
#   workflows do not cover, while optionally letting you replicate packaging
#   (macOS bundle / Windows build) and perform parity validation.
#
# Core (default) pipeline:
#   1. Enforce clean state & branch (main) unless overridden.
#   2. Derive current + target version (semantic bump or explicit).
#   3. Update crate versions (cargo-workspaces if available; manual fallback).
#   4. Update README version badge (unless --no-readme).
#   5. Run QA: rustfmt (check), clippy (deny warnings), cargo audit (unless disabled),
#      and tests (unless skipped).
#   6. Commit + create annotated tag (unless --no-tag / --dry-run).
#   7. Optionally push.
#
# Optional additive features (flags):
#   * Local packaging replication (macOS, Windows).
#   * Bundled macOS validation vs an existing release.
#   * Release notes & changelog generation.
#   * Metadata JSON emission.
#   * Diff summary printing.
#
# The authoritative public release artifacts are still produced by GitHub
# workflows on tag push. This script should remain lean and predictable.
#
# USAGE:
#   scripts/release.sh <version|major|minor|patch> [flags]
#
# EXAMPLES:
#   scripts/release.sh patch --push
#   scripts/release.sh 0.2.0 --notes --metadata-json --no-tag
#   scripts/release.sh minor --package-macos --validate-against last
#   scripts/release.sh patch --package-all --push
#
# FLAGS:
#   --push                Push commit + tag to origin.
#   --no-readme           Skip README badge version replacement.
#   --no-tests            Skip tests.
#   --no-qa               Skip rustfmt & clippy (tests still run unless also --no-tests).
#   --no-audit            Skip cargo audit (audit runs by default if QA enabled).
#   --fast                Shortcut: implies --no-tests --no-qa --no-audit.
#   --dry-run             Print actions; do not mutate repo / create tag.
#   --no-tag              Perform all steps except creating the tag.
#   --allow-dirty         Allow dirty working tree.
#   --allow-non-main      Skip branch==main enforcement.
#   --notes               Generate RELEASE_NOTES_<version>.md (committed unless --no-tag / --dry-run).
#   --diff                Print concise commit summary since previous tag.
#   --metadata-json       Emit release_meta_<version>.json (committed if tagging).
#   --changelog           Append/update CHANGELOG.md with a new top section for this version.
#   --package-macos       Build & bundle macOS desktop app (uses scripts/macos/bundle.sh).
#   --package-windows     Attempt Windows target build (no zip script yet).
#   --package-all         Shorthand for --package-macos --package-windows.
#   --bundle              Alias for --package-macos.
#   --sign-identity <ID>  Pass signing identity to macOS bundler (default ad-hoc '-').
#   --validate-against <version|last>
#                         After (or before, if no packaging) packaging, run local parity validator.
#
# EXIT CODES:
#   0 success
#   Non-zero on failure (see stderr for context)
#
set -euo pipefail
# Disable git pager to prevent interactive 'less' or SIGPIPE exits in non‑interactive or dry-run scenarios.
export GIT_PAGER=cat
export PAGER=cat
unset LESS 2>/dev/null || true

# In some dry-run cases (especially when piping git log output) a SIGPIPE from a pager
# or an unexpected non-zero from a subshell could cause the script to exit after
# generating notes. We trap that so a dry run still exits 0 when its core steps succeed.
DRY_RUN_NOTES_OK=0

########################################
# Logging helpers
########################################
color() { [[ -t 1 ]] && printf "\033[%sm%s\033[0m" "$1" "$2" || printf "%s" "$2"; }
bold()  { color 1 "$*"; }
info()  { printf "🔧 %s\n" "$*"; }
warn()  { printf "⚠️  %s\n" "$*" >&2; }
err()   { printf "❌ %s\n" "$*" >&2; }
ok()    { printf "✅ %s\n" "$*"; }
debug() { [[ "${VERBOSE:-0}" == "1" ]] && printf "🛈 %s\n" "$*" || true; }

########################################
# Defaults / Flags
########################################
DRY_RUN=0
DO_PUSH=0
ALLOW_DIRTY=0
ALLOW_NON_MAIN=0
RUN_QA=1
RUN_TESTS=1
RUN_AUDIT=1
UPDATE_README=1
CREATE_TAG=1
GENERATE_NOTES=0
PRINT_DIFF=0
WRITE_METADATA=0
UPDATE_CHANGELOG=0
PACKAGE_MACOS=0
PACKAGE_WINDOWS=0
VALIDATE_VERSION=""
FAST=0
SIGN_IDENTITY="-"
VERBOSE=0

TARGET_VERSION=""
BUMP_SPEC=""
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

########################################
# Usage / Help
########################################
if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  sed -n '1,/^set -euo pipefail/p' "$0"
  exit 0
fi

########################################
# Parse positional (version/bump)
########################################
if [[ $# -eq 0 ]]; then
  err "Missing version or bump spec (major|minor|patch|<semver>). Use --help for details."
  exit 1
fi

first="$1"
shift

if [[ "$first" =~ ^(major|minor|patch)$ ]]; then
  BUMP_SPEC="$first"
elif [[ "$first" =~ ^v?[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  TARGET_VERSION="${first#v}"
else
  err "Unrecognized version/bump token: $first"
  exit 1
fi

########################################
# Parse flags
########################################
while [[ $# -gt 0 ]]; do
  case "$1" in
    --push) DO_PUSH=1 ;;
    --no-readme) UPDATE_README=0 ;;
    --no-tests) RUN_TESTS=0 ;;
    --no-qa) RUN_QA=0 ;;
    --no-audit) RUN_AUDIT=0 ;;
    --fast) FAST=1; RUN_TESTS=0; RUN_QA=0; RUN_AUDIT=0 ;;
    --dry-run) DRY_RUN=1 ;;
    --no-tag) CREATE_TAG=0 ;;
    --allow-dirty) ALLOW_DIRTY=1 ;;
    --allow-non-main) ALLOW_NON_MAIN=1 ;;
    --notes) GENERATE_NOTES=1 ;;
    --diff) PRINT_DIFF=1 ;;
    --metadata-json) WRITE_METADATA=1 ;;
    --changelog) UPDATE_CHANGELOG=1 ;;
    --package-macos|--bundle) PACKAGE_MACOS=1 ;;
    --package-windows) PACKAGE_WINDOWS=1 ;;
    --package-all) PACKAGE_MACOS=1; PACKAGE_WINDOWS=1 ;;
    --validate-against)
      shift
      [[ $# -gt 0 ]] || { err "--validate-against requires an argument"; exit 1; }
      VALIDATE_VERSION="$1"
      ;;
    --sign-identity)
      shift
      [[ $# -gt 0 ]] || { err "--sign-identity requires an identity string"; exit 1; }
      SIGN_IDENTITY="$1"
      ;;
    --verbose) VERBOSE=1 ;;
    -h|--help)
      sed -n '1,/^set -euo pipefail/p' "$0"
      exit 0
      ;;
    *)
      err "Unknown flag: $1"
      exit 1
      ;;
  esac
  shift
done

########################################
# Helper: run command (respect DRY_RUN)
########################################
run() {
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "DRY: $*"
  else
    eval "$@"
  fi
}

########################################
# Trap (only mention tag cleanup if we created one)
########################################
TAG_CREATED=0
cleanup_trap() {
  if [[ $? -ne 0 ]]; then
    if [[ "${TAG_CREATED}" -eq 1 ]]; then
      err "Failure occurred after tag creation. You may need to: git tag -d v${TARGET_VERSION} && git reset --hard HEAD~1"
    else
      err "Failure occurred. Use git restore or git reset as needed."
    fi
  fi
}
trap cleanup_trap EXIT

########################################
# Preconditions
########################################
CURRENT_BRANCH="$(git rev-parse --abbrev-ref HEAD)"
if [[ "${ALLOW_NON_MAIN}" -eq 0 && "${CURRENT_BRANCH}" != "main" ]]; then
  err "Releases are expected from 'main' (current: ${CURRENT_BRANCH}). Use --allow-non-main to override."
  exit 1
fi

if [[ "${ALLOW_DIRTY}" -eq 0 && -n "$(git status --porcelain)" ]]; then
  err "Working tree not clean. Commit/stash or use --allow-dirty."
  exit 1
fi

########################################
# Derive current version
########################################
derive_current_version() {
  if command -v jq >/dev/null 2>&1; then
    cargo metadata --no-deps --format-version=1 \
      | jq -r '.packages[] | select(.name=="looplace-ui") | .version' \
      | head -n1
  else
    awk -F= '/^version *= *"/{gsub(/"/,"",$2);gsub(/^[ \t]+|[ \t]+$/,"",$2);print $2;exit}' ui/Cargo.toml
  fi
}
CURRENT_VERSION="$(derive_current_version || true)"
[[ -n "${CURRENT_VERSION}" ]] || { err "Unable to derive current version (looplace-ui)."; exit 1; }

########################################
# Compute target version if bump spec
########################################
if [[ -n "${BUMP_SPEC}" ]]; then
  IFS='.' read -r MA MI PA <<< "${CURRENT_VERSION}"
  case "${BUMP_SPEC}" in
    major) ((MA+=1)); MI=0; PA=0 ;;
    minor) ((MI+=1)); PA=0 ;;
    patch) ((PA+=1)) ;;
  esac
  TARGET_VERSION="${MA}.${MI}.${PA}"
fi

[[ -n "${TARGET_VERSION}" ]] || { err "Failed to resolve target version."; exit 1; }
TAG="v${TARGET_VERSION}"

########################################
# Previous tag (for notes/diff)
########################################
PREV_TAG="$(git describe --tags --abbrev=0 2>/dev/null || true)"
if [[ -z "${PREV_TAG}" ]]; then
  warn "No previous tag found (first release?)."
fi

########################################
# Summary Plan
########################################
cat <<EOF
------------------------------------------------------------
Looplace Release Plan
  Current version : ${CURRENT_VERSION}
  Target version  : ${TARGET_VERSION}
  Tag             : ${TAG} (create: $([[ ${CREATE_TAG} -eq 1 ]] && echo yes || echo no))
  Push            : $([[ ${DO_PUSH} -eq 1 ]] && echo yes || echo no)
  README badge    : $([[ ${UPDATE_README} -eq 1 ]] && echo yes || echo no)
  QA (fmt/clippy) : $([[ ${RUN_QA} -eq 1 ]] && echo yes || echo no)
  Tests           : $([[ ${RUN_TESTS} -eq 1 ]] && echo yes || echo no)
  Audit (cargo)   : $([[ ${RUN_AUDIT} -eq 1 ]] && echo yes || echo no)
  Notes file      : $([[ ${GENERATE_NOTES} -eq 1 ]] && echo yes || echo no)
  Diff print      : $([[ ${PRINT_DIFF} -eq 1 ]] && echo yes || echo no)
  Changelog       : $([[ ${UPDATE_CHANGELOG} -eq 1 ]] && echo yes || echo no)
  Metadata JSON   : $([[ ${WRITE_METADATA} -eq 1 ]] && echo yes || echo no)
  Package macOS   : $([[ ${PACKAGE_MACOS} -eq 1 ]] && echo yes || echo no)
  Package Windows : $([[ ${PACKAGE_WINDOWS} -eq 1 ]] && echo yes || echo no)
  Validate bundle : $([[ -n "${VALIDATE_VERSION}" ]] && echo "${VALIDATE_VERSION}" || echo no)
  Dry run         : $([[ ${DRY_RUN} -eq 1 ]] && echo yes || echo no)
  Fast mode       : $([[ ${FAST} -eq 1 ]] && echo yes || echo no)
  Prev tag        : ${PREV_TAG:-<none>}
------------------------------------------------------------
EOF

########################################
# Version Update
########################################
update_versions() {
  info "Updating crate versions to ${TARGET_VERSION}"
  if command -v cargo-workspaces >/dev/null 2>&1; then
    run cargo workspaces version \
      --force '*' \
      --exact \
      --no-git-commit \
      --no-git-tag \
      -y "${TARGET_VERSION}"
  else
    warn "cargo-workspaces not found; performing manual patch (internal dependency versions not rewritten)."
    local pattern='^version *= *"[0-9]+\.[0-9]+\.[0-9]+"'
    while IFS= read -r file; do
      if grep -q '^name *= *"looplace-' "$file"; then
        if grep -Eq "${pattern}" "$file"; then
          if [[ "${DRY_RUN}" -eq 1 ]]; then
            echo "DRY: sed update version in $file"
          else
            sed -i.bak -E "s/${pattern}/version = \"${TARGET_VERSION}\"/" "$file" && rm -f "${file}.bak"
          fi
        fi
      fi
    done < <(find . -type f -name Cargo.toml)
  fi
}

########################################
# README Badge
########################################
update_readme_badge() {
  [[ "${UPDATE_README}" -eq 1 ]] || { info "Skipping README badge"; return; }
  [[ -f README.md ]] || { warn "README.md missing; badge skip."; return; }
  local pattern='(version-)[0-9]+\.[0-9]+\.[0-9]+(-orange\.svg)'
  if grep -Eq "${pattern}" README.md; then
    if [[ "${DRY_RUN}" -eq 1 ]]; then
      echo "DRY: update README badge to ${TARGET_VERSION}"
    else
      sed -i.bak -E "s/${pattern}/\1${TARGET_VERSION}\2/" README.md && rm -f README.md.bak
    fi
  else
    warn "Version badge pattern not found in README."
  fi
}

########################################
# QA: fmt / clippy / audit
########################################
run_qa() {
  [[ "${RUN_QA}" -eq 1 ]] || { info "Skipping fmt/clippy (--no-qa or --fast)"; return; }

  info "rustfmt (check)"
  run cargo fmt --all -- --check

  info "clippy (deny warnings)"
  run cargo clippy --workspace --all-targets -- -D warnings

  if [[ "${RUN_AUDIT}" -eq 1 ]]; then
    info "cargo audit (pre-check)"
    if ! command -v cargo-audit >/dev/null 2>&1; then
      info "Installing cargo-audit (may take a moment)"
      run cargo install cargo-audit --locked
    fi
    if [[ "${DRY_RUN}" -eq 1 ]]; then
      echo "DRY: cargo audit -q"
    else
      # If audit fails, show verbose output inside group
      cargo audit -q || (warn "Audit failed; rerunning verbose"; cargo audit; exit 1)
    fi
  else
    info "Skipping cargo audit (--no-audit)"
  fi
}

########################################
# Tests
########################################
run_tests() {
  [[ "${RUN_TESTS}" -eq 1 ]] || { info "Skipping tests (--no-tests or --fast)"; return; }
  info "Running tests"
  run cargo test --workspace
}

########################################
# Commit & Tag
########################################
commit_and_tag() {
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    info "Dry run: skipping commit/tag creation."
    return
  fi

  if [[ -n "$(git status --porcelain)" ]]; then
    run git add .
    local msg="release: v${TARGET_VERSION}"
    run git commit -m "${msg}"
  else
    warn "No changes detected after version update."
  fi

  if [[ "${CREATE_TAG}" -eq 1 ]]; then
    # Pre-flight: ensure tag not present
    if git rev-parse "${TAG}" >/dev/null 2>&1; then
      err "Tag ${TAG} already exists locally."
      exit 1
    fi
    if git ls-remote --exit-code --tags origin "refs/tags/${TAG}" >/dev/null 2>&1; then
      err "Tag ${TAG} already exists remotely."
      exit 1
    fi
    run git tag -a "${TAG}" -m "Looplace ${TARGET_VERSION}"
    TAG_CREATED=1
    info "Created tag ${TAG}"
  else
    info "Skipping tag creation (--no-tag)"
  fi

  if [[ "${DO_PUSH}" -eq 1 ]]; then
    info "Pushing commit and tags"
    run git push --follow-tags origin HEAD
  else
    info "Skipping push (--push to enable)"
  fi
}

########################################
# Release Notes
########################################
generate_notes() {
  [[ "${GENERATE_NOTES}" -eq 1 ]] || return
  local notes_file="RELEASE_NOTES_${TARGET_VERSION}.md"
  info "Generating release notes: ${notes_file}"

  local range
  if [[ -n "${PREV_TAG}" ]]; then
    range="${PREV_TAG}..HEAD"
  else
    range="HEAD"
  fi

  local header="## Looplace ${TARGET_VERSION}"
  local date_line="$(date -u +'%Y-%m-%d UTC')"

  {
    echo "${header}"
    echo "_${date_line}_"
    echo
    git log --pretty=format:'* %s (%h)' ${range}
    echo
  } > "${notes_file}"

  if [[ "${DRY_RUN}" -eq 1 || "${CREATE_TAG}" -eq 0 ]]; then
    info "Notes generated (not auto-committed due to dry-run/no-tag)."
  else
    run git add "${notes_file}"
    run git commit --amend --no-edit || true
  fi
}

########################################
# Diff summary
########################################
print_diff_summary() {
  [[ "${PRINT_DIFF}" -eq 1 ]] || return
  local range
  if [[ -n "${PREV_TAG}" ]]; then
    range="${PREV_TAG}..HEAD"
  else
    range="HEAD"
  fi
  echo "---- Commit Summary (${range}) ----"
  git log --oneline ${range}
  echo "-----------------------------------"
}

########################################
# Changelog update
########################################
update_changelog() {
  [[ "${UPDATE_CHANGELOG}" -eq 1 ]] || return
  local file="CHANGELOG.md"
  info "Updating changelog (${file})"

  local range
  if [[ -n "${PREV_TAG}" ]]; then
    range="${PREV_TAG}..HEAD"
  else
    range="HEAD"
  fi

  local tmp="$(mktemp)"
  local date_line
  date_line="$(date -u +'%Y-%m-%d')"

  {
    echo "## ${TARGET_VERSION} - ${date_line}"
    git log --pretty=format:'* %s (%h)' ${range}
    echo
    if [[ -f "${file}" ]]; then
      cat "${file}"
    else
      echo "_Initial changelog created._"
      echo
    fi
  } > "${tmp}"

  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "DRY: would update ${file}"
    rm -f "${tmp}"
  else
    mv "${tmp}" "${file}"
    if [[ "${CREATE_TAG}" -eq 1 ]]; then
      run git add "${file}"
      run git commit --amend --no-edit || true
    fi
  fi
}

########################################
# Metadata JSON
########################################
emit_metadata_json() {
  [[ "${WRITE_METADATA}" -eq 1 ]] || return
  local file="release_meta_${TARGET_VERSION}.json"
  info "Writing metadata JSON (${file})"

  local sha
  sha="$(git rev-parse HEAD)"
  local crates_changed
  crates_changed="$(git diff --name-only ${PREV_TAG:+${PREV_TAG}} HEAD | grep -E 'Cargo.toml$' || true)"

  local ts
  ts="$(date -u +'%Y-%m-%dT%H:%M:%SZ')"

  {
    echo "{"
    echo "  \"version\": \"${TARGET_VERSION}\","
    echo "  \"tag\": \"${TAG}\","
    echo "  \"git_sha\": \"${sha}\","
    echo "  \"previous_tag\": \"${PREV_TAG}\","
    echo "  \"timestamp_utc\": \"${ts}\","
    echo "  \"crates_changed\": ["
    if [[ -n "${crates_changed}" ]]; then
      while IFS= read -r line; do
        printf '    "%s",\n' "${line}"
      done <<< "${crates_changed}" | sed '$ s/,$//'
    fi
    echo "  ]"
    echo "}"
  } > "${file}"

  if [[ "${DRY_RUN}" -eq 1 || "${CREATE_TAG}" -eq 0 ]]; then
    info "Metadata JSON generated (not committed due to dry-run/no-tag)."
  else
    run git add "${file}"
    run git commit --amend --no-edit || true
  fi
}

########################################
# Packaging (macOS)
########################################
package_macos() {
  [[ "${PACKAGE_MACOS}" -eq 1 ]] || return
  if [[ "$(uname -s)" != "Darwin" ]]; then
    warn "macOS packaging requested but host is not Darwin; skipping."
    return
  fi
  local target_triple="aarch64-apple-darwin"
  info "Building macOS target (${target_triple})"
  run cargo build --release -p looplace-desktop --features desktop --target "${target_triple}"

  local out_basename="looplace-desktop-${TAG}-${target_triple}"
  info "Bundling macOS app (OUT_BASENAME=${out_basename})"
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "DRY: RUST_TARGET=${target_triple} OUT_BASENAME=${out_basename} SIGN_IDENTITY='${SIGN_IDENTITY}' scripts/macos/bundle.sh"
  else
    RUST_TARGET="${target_triple}" \
    OUT_BASENAME="${out_basename}" \
    OUTPUT_DIR="dist" \
    SIGN_IDENTITY="${SIGN_IDENTITY}" \
    STRICT=1 \
    scripts/macos/bundle.sh
  fi
}

########################################
# Packaging (Windows)
########################################
package_windows() {
  [[ "${PACKAGE_WINDOWS}" -eq 1 ]] || return
  local target_triple="x86_64-pc-windows-msvc"
  # Ensure target is installed
  if ! rustup target list --installed | grep -qx "${target_triple}"; then
    warn "Windows target (${target_triple}) not installed; install with: rustup target add ${target_triple}"
    return
  fi
  info "Building Windows target (${target_triple})"
  run cargo build --release -p looplace-desktop --features desktop --target "${target_triple}"
  info "Windows build complete (packaging handled by CI release workflow)."
}

########################################
# Validation (macOS parity)
########################################
validate_bundle() {
  [[ -n "${VALIDATE_VERSION}" ]] || return
  local version_arg="${VALIDATE_VERSION}"
  if [[ "${version_arg}" == "last" ]]; then
    if [[ -z "${PREV_TAG}" ]]; then
      warn "No previous tag to validate against."
      return
    fi
    version_arg="${PREV_TAG#v}"
  fi
  if [[ "$(uname -s)" != "Darwin" ]]; then
    warn "Validation requested but not on macOS; skipping."
    return
  fi
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "DRY: scripts/validate_bundle_local.sh --version ${version_arg} --skip-build"
    return
  fi
  if [[ ! -x scripts/validate_bundle_local.sh ]]; then
    warn "Validator script missing or not executable (scripts/validate_bundle_local.sh); skipping."
    return
  fi
  info "Validating local macOS bundle parity vs version ${version_arg}"
  # If we already packaged macOS, we can skip build inside validator
  if [[ "${PACKAGE_MACOS}" -eq 1 ]]; then
    scripts/validate_bundle_local.sh --version "${version_arg}" --skip-build || {
      err "Bundle validation failed."
      exit 1
    }
  else
    scripts/validate_bundle_local.sh --version "${version_arg}" || {
      err "Bundle validation failed."
      exit 1
    }
  fi
}

########################################
# EXECUTION ORDER
########################################
update_versions
update_readme_badge
run_qa
run_tests
print_diff_summary
commit_and_tag
# Guard notes generation; if it fails during a dry run we don't want to abort the whole script.
if ! generate_notes; then
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    warn "Non-fatal: release notes generation failed during dry run."
    DRY_RUN_NOTES_OK=1
  else
    err "Release notes generation failed."
    exit 1
  fi
fi
update_changelog
emit_metadata_json
package_macos
package_windows
validate_bundle

ok "Release preflight complete (target version ${TARGET_VERSION})."

if [[ "${CREATE_TAG}" -eq 1 && "${DO_PUSH}" -eq 0 ]]; then
  echo
  echo "NEXT: push the tag to trigger CI release:"
  echo "  git push origin ${TAG}"
fi

if [[ "${DRY_RUN}" -eq 1 ]]; then
  if [[ "${DRY_RUN_NOTES_OK}" -eq 1 ]]; then
    warn "Dry run completed with a non-fatal notes generation issue (ignored)."
  else
    warn "Dry run: no repository changes or tags were created."
  fi
fi

# End of file

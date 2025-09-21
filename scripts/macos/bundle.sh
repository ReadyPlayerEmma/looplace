#!/usr/bin/env bash
set -euo pipefail

APP_NAME="Looplace"
PROFILE="${PROFILE:-release}"
OUTPUT_DIR="target/bundle"
INFO_PLIST="desktop/macos/Info.plist"
SIGN_IDENTITY="${SIGN_IDENTITY:--}" # default to ad-hoc signing

# Try to locate the compiled binary (prefer target/<triple>/PROFILE/desktop, fallback to target/PROFILE/desktop)
BIN_PATH=""
if [[ -n "${RUST_TARGET:-}" ]] && [[ -f "target/${RUST_TARGET}/${PROFILE}/desktop" ]]; then
  BIN_PATH="target/${RUST_TARGET}/${PROFILE}/desktop"
fi

if [[ -z "$BIN_PATH" ]]; then
  BIN_PATH=$(find "target" -maxdepth 3 -path "*/${PROFILE}/desktop" -type f | head -n1 || true)
fi

if [[ -z "$BIN_PATH" ]]; then
  echo "❌ Could not find compiled desktop binary. Run 'cargo build --release -p desktop' first." >&2
  exit 1
fi

if [[ ! -f "$INFO_PLIST" ]]; then
  echo "❌ Missing Info.plist template at $INFO_PLIST" >&2
  exit 1
fi

BUNDLE_ROOT="${OUTPUT_DIR}/${APP_NAME}.app"
MACOS_DIR="${BUNDLE_ROOT}/Contents/MacOS"
RESOURCES_DIR="${BUNDLE_ROOT}/Contents/Resources"

rm -rf "$BUNDLE_ROOT"
mkdir -p "$MACOS_DIR" "$RESOURCES_DIR"

cp "$BIN_PATH" "$MACOS_DIR/${APP_NAME}"
chmod +x "$MACOS_DIR/${APP_NAME}"

cp "$INFO_PLIST" "$BUNDLE_ROOT/Contents/Info.plist"

if [[ -d desktop/assets ]]; then
  cp -R desktop/assets "$MACOS_DIR/assets"
fi

VERSION=$(grep -m1 '^version' desktop/Cargo.toml | sed 's/.*"\(.*\)"/\1/')
if command -v /usr/libexec/PlistBuddy >/dev/null 2>&1; then
  /usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $VERSION" "$BUNDLE_ROOT/Contents/Info.plist"
  /usr/libexec/PlistBuddy -c "Set :CFBundleVersion $VERSION" "$BUNDLE_ROOT/Contents/Info.plist"
fi


if command -v xattr >/dev/null 2>&1; then
  xattr -cr "$BUNDLE_ROOT" || true
fi

if command -v codesign >/dev/null 2>&1; then
  codesign --force --deep --sign "$SIGN_IDENTITY" "$BUNDLE_ROOT" || true
fi

mkdir -p "$OUTPUT_DIR"
BUNDLE_PARENT="${OUTPUT_DIR}/${APP_NAME}-macos"
rm -rf "$BUNDLE_PARENT"
mkdir -p "$BUNDLE_PARENT"
mv "$BUNDLE_ROOT" "$BUNDLE_PARENT/"

BUNDLE_ZIP="${OUTPUT_DIR}/${APP_NAME}-macos.zip"
rm -f "$BUNDLE_ZIP"
ditto -c -k --keepParent "$BUNDLE_PARENT" "$BUNDLE_ZIP"

mv "$BUNDLE_PARENT/${APP_NAME}.app" "$BUNDLE_ROOT"
rm -rf "$BUNDLE_PARENT"

echo "✅ Bundle created at ${BUNDLE_ZIP}"

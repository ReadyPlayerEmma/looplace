# Agent Notes

## Project context
- Looplace is a Dioxus-generated workspace; treat the existing module layout as canonical.
- Logic lives in `ui/`; platform crates (`web/`, `desktop/`, `mobile/`) stay as thin launchers/glue.
- Server functions in `api/` remain stubbed during the front-end demo (no real backend calls yet).
- Health/data layer is **native-only**: `looplace-libre/` (FreeStyle Libre 2 driver) and `looplace-store/` (unified Parquet store) can't be hard deps of `ui/` (it compiles to wasm) — in `ui/` they're gated to desktop OSes, and web/mobile show a desktop-only note. The four device keys live in `looplace-libre-keys/` behind a feature flag, never in default builds.
- Cognition tasks (PVT, 2-back) and the Results UI are shipped. Current focus: the **health vertical** — see **M6** in `TODO.md` (glucose via the native Libre 2 driver has landed; next is glucose UX polish, then correlation).

## Quick start for new agents
- **Local dev**
  - Web: `cd web && dx serve --platform web --open`
  - Desktop: `cargo run -p looplace-desktop --features desktop` (reliable). `dx serve --platform desktop` from `desktop/` also works, but only if the dioxus-cli `dx` is first on PATH — a Deno (or other) `dx` will reject `--platform`.
- **Release builds**
  - `cargo build --release -p looplace-desktop --features desktop --target aarch64-apple-darwin`
  - `cargo build --release -p looplace-desktop --target x86_64-pc-windows-msvc`
- **Bundling**
  - macOS `.app`: `./scripts/macos/bundle.sh` (CI-first; emits `dist/<OUT_BASENAME>/Looplace.app` + `dist/<OUT_BASENAME>.zip`, strips AppleDouble metadata, ad-hoc signed with `SIGN_IDENTITY=-` by default)
  - Local parity check: `scripts/validate_bundle_local.sh --version <X.Y.Z>` (compares structure vs published release; ignores `._*` forks)
  - Windows portable zip: generated automatically in CI (`Looplace.exe` + `assets/`); future Windows packaging script TBD.

## Local release script
Use `scripts/release.sh <patch|minor|major|X.Y.Z> [flags]` as a preflight helper (version bump + QA + tag). CI still produces the canonical release artifacts after the tag is pushed.

Common:
- Tag & push patch: `scripts/release.sh patch --push`
- Preview minor bump w/ notes (no tag): `scripts/release.sh minor --notes --metadata-json --no-tag`
- Mac bundle + parity check vs last: `scripts/release.sh patch --package-macos --validate-against last`
- Fast dry-run: `scripts/release.sh patch --fast --dry-run`

Key flags (additive):
- `--fast` (skip tests, QA, audit)
- `--no-tests`, `--no-qa`, `--no-audit`, `--no-readme`
- `--notes`, `--diff`, `--metadata-json`, `--changelog`
- `--package-macos` / `--bundle`, `--package-windows`, `--package-all`
- `--validate-against <version|last>`
- `--sign-identity <ID>`
- `--allow-dirty`, `--allow-non-main`, `--no-tag`, `--push`

macOS packaging uses the same bundler as CI; Windows build is a convenience (artifact still produced authoritatively in CI).

## CI snapshot
- `Build (Desktop)`: macOS Apple Silicon `.app` bundle + Windows x64 zip on every push/PR (now both use canonical macOS bundler script).
- `Release (Desktop)`: tagged builds publish both artifacts to GitHub Releases.
- `Deploy (CF Pages)`: disabled for now; rename `.github/workflows/deploy-pages.yml.disabled` back to `.yml` if we resume web builds (requires `CF_API_TOKEN`/`CF_ACCOUNT_ID`).

## Working guidelines
- Stick to existing architectural patterns; defer to `CONTRIBUTING.md` and `TODO.md` before inventing new abstractions.
- UI code in `ui/` must stay platform-agnostic; call platform APIs only from the platform crates.
- Timing-sensitive engines rely on `requestAnimationFrame`/`performance.now()` wrappers—preserve that contract.
- When changing UX or visuals, ask the user to run a manual smoke test (PVT focus, bundling, etc.).
- Keep asset paths using the `asset!` macro; native builds expect files alongside the binary/bundle.

## Known quirks & tips
- **hidapi on macOS must run on one long-lived thread.** The `IOHIDManager` is pinned to the `CFRunLoop` of the thread that created it; driving it from a fresh per-call `std::thread` crashes on the *second* use (`EXC_BREAKPOINT` in `__CFCheckCFInfoPACSignature`). All Libre device I/O is serialized onto `ui/src/core/glucose.rs::device_thread` — never call the driver off that thread.
- macOS binaries are ad-hoc signed; first launch may need `xattr -cr Looplace.app` or right-click → Open.
- Windows zip expects `Looplace.exe` at the root with an `assets/` folder; keep that layout stable.
- `ui/src/navbar.rs` inlines CSS for release builds—maintain parity if adding new global styles.
- Windows WebView2 drops `autofocus` on dynamically inserted nodes; capture the mounted PVT hitbox (`MountedEvent`) and call `set_focus(true)` via `dioxus::prelude::spawn` whenever runs start or advance so keyboard input stays live.
- Canonical macOS bundling: `scripts/macos/bundle.sh` (env vars of note: `RUST_TARGET`, `OUT_BASENAME`, `OUTPUT_DIR`, `SIGN_IDENTITY`, `STRICT=1` for structure checks; removes AppleDouble `._*` and `.DS_Store`).
- Parity validation (macOS): `scripts/validate_bundle_local.sh --version <X.Y.Z>` compares a published artifact with a freshly packaged local one.
- Optional checksum (manual): `shasum -a 256 dist/*.zip` for release notes.
- Remember to update docs (`README.md`, `TODO.md`, `AGENTS.md`) whenever workflows or roadmaps shift.
- When release smoke uncovers issues, use `gh issue view`/`gh issue comment` to triage and reply quickly from the CLI; note key repro steps and request retests once fixes land.

## Research + questions backlog
- Record any Dioxus API changes or workarounds discovered mid-task.
- Surface uncertainties about wiring 2-back metrics into the Results experience (trend charts, exports) early.
- Windows desktop polish: research setting custom window title & icon for Dioxus 0.6.x (issue #5). Goals: (1) runtime window title “Looplace”, (2) window icon via `WindowBuilder::with_window_icon` (or current equivalent) loading a multi-size `.ico`, (3) executable/icon embedding for Explorer (e.g. `winres` or `.rc` file) so the binary itself shows the Looplace icon, (4) asset pipeline for generating a multi-resolution `.ico` (16/32/48/64/128/256) plus fallback PNG if needed, (5) confirm high-DPI & dark mode behavior. Output: step-by-step implementation plan, required crate additions, and verification checklist.

## Parking lot
- Add TODO clarifications or follow-ups here as they emerge.
- Note documentation gaps or onboarding pain points to circle back on.

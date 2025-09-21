# Agent Notes

## Project context
- Looplace is a Dioxus-generated workspace; treat the existing module layout as canonical.
- Logic lives in `ui/`; platform crates (`web/`, `desktop/`, `mobile/`) stay as thin launchers/glue.
- Server functions in `api/` remain stubbed during the front-end demo (no real backend calls yet).
- Current milestone to tackle next: **M2 — Results UI** (lists, charts, export) now that the 2-back task is live.

## Quick start for new agents
- **Local dev**
  - Web: `cd web && dx serve --platform web --open`
  - Desktop: `cd desktop && dx serve --platform desktop` (desktop feature enabled by default)
- **Release builds**
  - `cargo build --release -p desktop --features desktop --target aarch64-apple-darwin`
  - `cargo build --release -p desktop --target x86_64-pc-windows-msvc`
- **Bundling**
  - macOS `.app`: `./scripts/macos/bundle.sh` (drops `target/bundle/Looplace.app` + zip, ad-hoc signed)
  - Windows portable zip generated automatically in CI (`Looplace.exe` + `assets/`)

## CI snapshot
- `Build (Desktop)`: macOS Apple Silicon `.app` bundle + Windows x64 zip on every push/PR.
- `Release (Desktop)`: tagged builds publish both artifacts to GitHub Releases.
- `Deploy (CF Pages)`: manual `workflow_dispatch` (set `CF_API_TOKEN`/`CF_ACCOUNT_ID` before enabling push deploys).

## Working guidelines
- Stick to existing architectural patterns; defer to `README.md`/`TODO.md` before inventing new abstractions.
- UI code in `ui/` must stay platform-agnostic; call platform APIs only from the platform crates.
- Timing-sensitive engines rely on `requestAnimationFrame`/`performance.now()` wrappers—preserve that contract.
- When changing UX or visuals, ask the user to run a manual smoke test (PVT focus, bundling, etc.).
- Keep asset paths using the `asset!` macro; native builds expect files alongside the binary/bundle.

## Known quirks & tips
- macOS binaries are ad-hoc signed; first launch may need `xattr -cr Looplace.app` or right-click → Open.
- Windows zip expects `Looplace.exe` at the root with an `assets/` folder; keep that layout stable.
- `ui/src/navbar.rs` inlines CSS for release builds—maintain parity if adding new global styles.
- The `scripts/macos/bundle.sh` script can take `SIGN_IDENTITY` env once Developer ID certificates return.
- Remember to update docs (`README.md`, `TODO.md`, `AGENTS.md`) whenever workflows or roadmaps shift.

## Research + questions backlog
- Record any Dioxus API changes or workarounds discovered mid-task.
- Surface uncertainties about wiring 2-back metrics into the Results experience (trend charts, exports) early.

## Parking lot
- Add TODO clarifications or follow-ups here as they emerge.
- Note documentation gaps or onboarding pain points to circle back on.

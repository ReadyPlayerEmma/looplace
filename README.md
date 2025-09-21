# Looplace

Small loops • clear minds.
Looplace is a cross-platform cognitive testing app built with **Rust** and **Dioxus**. We run as a WASM SPA on the web, with first-class paths to **desktop** (wry/WebView) and **mobile** (iOS/Android) without rewriting UI. Back-end work will land as **Dioxus server functions** (Axum under the hood), with a deployment target of **Cloudflare Workers + D1**.

---

## Why this layout?

We generated a Dioxus workspace via `dx` and then tailored it for Looplace:

```
├── api/             # server functions (shared “backend” crate)
├── desktop/         # desktop entry + platform glue
├── looplace-brand/  # brand assets
├── mobile/          # mobile entry + platform glue
├── ui/              # shared UI + client-side logic (PVT, n-back, metrics, storage)
├── web/             # web entry + platform glue
├── Cargo.toml       # workspace config
└── README.md        # you are here
```

**Design rule of thumb**

* **Most logic lives in `ui/`** (rendering, task engines, client metrics, storage, routing pieces that are common).
* **Server functions live in `api/`** (DB access, auth, summaries ingestion). They’re callable from `ui/` like ordinary async fns.
* Platform crates (`web/`, `desktop/`, `mobile/`) are **thin shells**: entrypoints, router wiring, and any platform-specific glue/assets.

---

## Quick start

### Prereqs

* Rust stable toolchain
* Dioxus CLI:

  ```bash
  cargo install dioxus-cli
  ```

### Run on web (SPA)

```bash
cd web
dx serve --platform web --open
```

### Run on desktop

```bash
cd desktop
dx serve --platform desktop
```

### Run on mobile (optional)

```bash
cd mobile
dx serve --platform ios      # iOS Simulator
dx serve --platform android  # Android; more setup required
```

> All builds reuse `ui/` so features land everywhere.

---

## Frontend architecture (CSR, timing-safe)

* **CSR only**—we don’t rely on SSR.
* Precise timing via `performance.now()` + `requestAnimationFrame` inside `ui/` task engines.
* **Crash safety**: trial data buffered in memory; on finish we compute metrics, then persist a single **summary** (localStorage now; D1 later).
* **Privacy by default**: no PII; summaries are anonymous unless a user opts in.

---

## Server functions (full-stack, Axum-based)

* Implemented in `api/` with `#[server(...)]`.
* During `dx serve`, server functions are collected into an Axum router for local development; the client can `api::my_fn(...).await` directly.
* **Cloudflare plan** (WIP): Cloudflare now demonstrates Axum on Workers. We’ll adapt `api/`’s Axum service to Workers and expose the same server function routes. The goal is: no client changes—functions still look like local async calls, just proxied to Workers.

### Example

**`api/src/lib.rs`**

```rust
use dioxus::prelude::*;

#[server(SaveSummary)]
pub async fn save_summary(json_payload: String) -> Result<(), ServerFnError> {
    // TODO: insert into D1 later; for now maybe log or noop
    Ok(())
}
```

**`ui/src/some_page.rs`**

```rust
let payload = serde_json::to_string(&summary)?;
if let Err(e) = api::save_summary(payload).await {
    log::warn!("Save failed (stub): {e}");
}
```

### Server-only dependencies

When you add things like SQL, queues, mailers, **make them server-only** (avoid compiling on WASM):

* Put them behind `cfg(feature = "server")` in `api/Cargo.toml`.
* Keep `ui/` free of `web-sys`/platform deps; put those in platform crates if needed.

---

## What lives where?

### `ui/` (shared, front-end only)

* **Task engines**:

  * `pvt/` – stimulus scheduler (ITI jitter), response capture, **RT stream**, QC
  * `nback/` – 2-back letter stream, balanced targets/lures, **d′/criterion** scoring
* **Metrics**:

  * **PVT**: median/mean RT, SD, p10/p90, **lapses ≥500 ms**, minor lapses 355–499 ms, false starts, **time-on-task slope**.
  * **N-back (2-back)**: hits/FA/miss/CR, **d′**, criterion, accuracy, reaction-time distribution.
* **Storage**:

  * Ephemeral trial data in memory only.
  * On finish: one **summary JSON** → localStorage (today) → server function (later).
* **QC**:

  * Visibility changes (tab out) → flag run.
  * Keyboard repeat/hold detection → flag anticipations.
  * Minimum trial count guard.

### `api/` (server functions)

* `save_summary` (stubbed) → later writes to D1
* `get_my_summaries` (stubbed) → later reads from D1
* Future: auth/session bootstrap, export CSV/JSON, minimal email notifications

### Platform crates

* **`web/`**: favicon/theme, global CSS, routing wrapper
* **`desktop/`**: window config, file export hooks, possible tray integration later
* **`mobile/`**: status bar/launch screen, vibration haptics, power-save hints

---

## Building blocks we’ll add next

* **Results view (ui/)**:

  * Sparkline of median RT over time
  * Lapses count bar, false starts, slope
  * 2-back d′ trend
  * CSV/JSON export (client-side)
* **Self-report (ui/)**: short mood/cog questionnaire (PHQ-2/GAD-2-style brevity, non-diagnostic), appended to summary.
* **Offline queue (ui/)**: if `save_summary` fails, store in an outbox and retry.

---

## Data model (v0)

```json
{
  "id": "pvt-2025-09-07T17:03:20Z-uuid",
  "task": "pvt",
  "created_at": "2025-09-07T17:03:20Z",
  "client": { "platform": "web|desktop|ios|android", "tz": "America/Chicago" },
  "metrics": {
    "median_rt_ms": 284.9,
    "mean_rt_ms": 301.2,
    "sd_rt_ms": 55.3,
    "p10_rt_ms": 230.0,
    "p90_rt_ms": 388.0,
    "lapses_ge_500ms": 1,
    "minor_lapses_355_499ms": 2,
    "false_starts": 0,
    "time_on_task_slope_ms_per_min": 12.4,
    "qc": { "visibility_blur_events": 0, "min_trials_met": true }
  },
  "notes": "Optional, client-only"
}
```

> Identical shape for 2-back, with `metrics` replaced by d′/criterion fields.

---

## Timing & accuracy guidelines

* Use **`requestAnimationFrame`** and **`performance.now()`** for all scheduling; never `setTimeout` for stimulus onset.
* Keep rendering lightweight during tests (no heavy DOM churn).
* Debounce visibility events; if the tab loses focus, mark QC and consider pausing.
* Prefer **keyboard events** to clicks on web; on mobile, support a large tap target.

---

## Accessibility & inclusion

* WCAG AA colors from the brand palette; high-contrast test view.
* Keyboard-only control is mandatory for web/desktop.
* The PVT pad auto-focuses on start so space/enter work immediately, and the cancel control stays clickable above the stimulus area.
* Clear instructions and a visible **pause/abort** affordance; no dark patterns.
* Plain-language summaries; no medical claims.

---

## Cloudflare path (high level, WIP)

1. **Develop locally** with `dx serve` (server functions via Axum).
2. **Export Axum router** from `api/` (or use the server-fn registry) and adapt to Workers using Cloudflare’s Axum-on-Workers example.
3. Map the server-function routes under `/api/*` and set the client **base URL** accordingly (DX fullstack respects a base).
4. Wire D1 (schema: `summaries` table) and Durable Objects if you want per-user queues.
5. Later: gate with a simple session cookie/JWT; still call from `ui/` as `api::fn().await`.

> Until the official “dx → Workers” path is turnkey, this keeps us unblocked while preserving the ergonomics of server functions.

---

## Dev workflow

* **Run web**: `cd web && dx serve --platform web --open`
* **Run desktop**: `cd desktop && dx serve --platform desktop`
* **Run mobile**: `cd mobile && dx serve --platform ios` (or `android`)
* **Lint/format**: `cargo clippy` / `cargo fmt`
* **Feature flags**: prefer small, descriptive `cfg(feature = "...")` toggles for experimental tasks.
* **Papercut sweep**: park recurring friction in the TODO log after each playtest so small regressions don’t pile up and dull the joy.

## CI & deployment

* `Build (Desktop)` runs on every push/PR and currently produces a release build for macOS Apple Silicon. Add more targets by expanding the workflow matrix when we are ready.
* `Release (Desktop)` fires on tags matching `v*.*.*`, zips the macOS Apple Silicon build, and attaches it to the corresponding GitHub release.
* `Deploy (CF Pages)` is opt-in via the Actions UI (`workflow_dispatch`); uncomment the push trigger in `.github/workflows/deploy-pages.yml` once continuous deploys are desired and ensure `CF_API_TOKEN` and `CF_ACCOUNT_ID` secrets are set.

---

## Contributing conventions

* Modules in `ui/` are **pure** and platform-agnostic; platform glue lives only in `web/`, `desktop/`, `mobile/`.
* Keep server-only crates out of `ui/` deps.
* New tasks: add `ui/tasks/<name>/` with `engine.rs`, `metrics.rs`, `view.rs`.
* Every task **must** emit a single summary JSON and never leak trial-level data off device by default.

---

## Roadmap

* **v0.1**: PVT (done), results list, local summaries, stubbed save
* **v0.2**: 2-back with d′, SVG trends, self-report
* **v0.3**: Cloudflare Worker + D1 (save/load), export API, minimal auth
* **v0.4**: Desktop packaging, iOS TestFlight build

---

## FAQ

**Why no SSR?**
Devices are fast; our UX is app-like; search crawlers execute JS fine. CSR keeps the stack simpler across web/desktop/mobile.

**Is timing good enough in WebView (desktop/mobile)?**
Yes—`performance.now()` is monotonic/high-res in modern WebViews. We still record QC flags (visibility, focus, event jitter).

**What about personal data?**
Looplace stores only test summaries by default. Users can export or delete local data anytime. No medical advice, no diagnosis.

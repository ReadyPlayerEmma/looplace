# Contributing to Looplace

Thanks for helping evolve Looplace. The project grows out of real-world needs to monitor cognition kindly, so we aim for a smooth contributor experience. This guide collects the essential development notes that keep the workspace consistent across platforms.

---

## Getting started

### Prerequisites

- Rust (stable toolchain)
- [Dioxus CLI](https://dioxuslabs.com/learn/0.6/getting_started/installation/)

```bash
cargo install dioxus-cli
```

### Local targets

```bash
# Web SPA
dx serve --platform web --open           # run from the `web/` crate

# Desktop (wry/WebView)
dx serve --platform desktop              # run from the `desktop/` crate

# Mobile (optional)
dx serve --platform ios                  # iOS simulator
dx serve --platform android              # Android — extra tooling required
```

All platforms pull UI and logic from the shared `ui/` crate, so changes propagate everywhere.

---

## Workspace layout

```
├── api/             # server functions (shared “backend” crate)
├── desktop/         # desktop entry + platform glue
├── looplace-brand/  # brand assets
├── mobile/          # mobile entry + platform glue
├── ui/              # shared UI + client-side logic (PVT, n-back, metrics, storage)
├── web/             # web entry + platform glue
├── Cargo.toml       # workspace config
└── README.md        # high-level overview
```

**Design principle**

- Core logic lives in `ui/`: rendering, task engines, metrics, storage, and cross-platform glue.
- `api/` collects Dioxus server functions. They are callable from `ui/` like normal async functions and can later target Cloudflare Workers.
- Platform crates (`web/`, `desktop/`, `mobile/`) stay thin: routing, entry points, platform-specific assets.

---

## Architecture notes

### Frontend

- CSR only—no server-side rendering.
- Timing-sensitive engines (PVT, 2-back) rely on `performance.now()` and `requestAnimationFrame` wrappers in `ui/core/timing.rs`.
- Trial data buffers in memory during runs; completing a task calculates metrics and persists a single summary record (localStorage today, server later).
- Privacy-first: summaries are anonymous unless someone opts in to share context.

### Server functions

- Implemented in `api/` using `#[server(...)]`.
- During `dx serve`, Dioxus exposes these as Axum routes so the client can `api::fn_name().await` directly.
- When adding server-only dependencies (SQL, queues, mailers), gate them with `cfg(feature = "server")` to avoid pulling them into WASM builds.
- Cloudflare Workers plan: adapt the Axum router produced by Dioxus to Workers so client calls remain unchanged.

---

## Module map

### `ui/`

- **Tasks**
  - `tasks/pvt/`: PVT engine, metrics, and view (ITI jitter, reaction stream, lapse flags).
  - `tasks/nback/`: 2-back engine with seeded letter stream, d′/criterion metrics, and immediate feedback.
- **Core utilities**: timing abstraction, local storage helpers, QC flags, platform detection, formatting.
- **Results**: list/detail placeholders ready for trend charts and exports.
- **Views**: route-level components imported by each platform.

### `api/`

- `save_summary` (stub) — later writes to D1 or another persistent store.
- `get_my_summaries` (stub) — placeholder for history lookups.
- Future: authentication bootstrap, exports, notifications.

### Platform crates

- `web/`: router wiring, global CSS/assets.
- `desktop/`: window config, resource resolution, future desktop-specific affordances.
- `mobile/`: status bar/launch handling, future haptics/power hints.

---

## Summary data model

Summaries are stored under the localStorage key `looplace_summaries` as a JSON array. Each entry is shaped like:

```json
{
  "id": "pvt-2025-09-07T17:03:20Z-uuid",
  "task": "pvt" | "nback2",
  "created_at": "2025-09-07T17:03:20Z",
  "client": { "platform": "web|desktop|ios|android", "tz": "America/Chicago" },
  "metrics": { /* task-specific fields */ },
  "qc": { "visibility_blur_events": 0, "min_trials_met": true, "notes": "" },
  "notes": ""
}
```

2-back runs include fields such as `hits`, `false_alarms`, `d_prime`, `criterion`, and hit reaction-time distribution. PVT runs supply reaction statistics, lapse counts, and slope values.

---

## Development guidelines

- Keep per-frame work light during tasks; avoid heavy DOM churn that could affect timing.
- Ensure keyboard shortcuts stay active (both tasks auto-focus their pads so space/enter fire immediately). Maintain large tap targets on touch devices.
- Provide clear abort/cancel affordances and visible feedback for responses—this is UX as much as data quality.
- Respect accessibility: high-contrast palettes, reduced-motion compliance, and descriptive copy.
- Refer to `TODO.md` for roadmap items (M2 Results UI, exports, etc.).
- Cloudflare deployment is currently paused. The workflow lives at `.github/workflows/deploy-pages.yml.disabled`; restore the filename plus `CF_API_TOKEN`/`CF_ACCOUNT_ID` secrets to re-enable.

---

## Need help?

If something feels unclear or brittle, add a note in `AGENTS.md` under “Parking lot” or open a discussion. We’re building this to support real people—tight feedback loops keep it trustworthy.

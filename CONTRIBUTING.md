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

> The workspace crates are published as `looplace-*` packages. When invoking `dx` from the workspace root, pass `--package looplace-desktop` (or `looplace-web`) to pick the right binary; running commands from inside each crate directory works without extra flags.

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

### UI design system

- The global palette and component tokens live in `web/assets/main.css` (mirrored in `desktop/assets/main.css`). Key variables:
  - `--color-primary` (`#f05a7e`) for primary CTA buttons.
  - `--color-accent` (`#1f68ff`) for secondary actions and informative highlights.
  - `--color-surface` / `--color-surface-strong` for layered cards.
- Buttons should use the shared classes:
  - `button button--primary` for main actions (start runs).
  - `button button--accent` for secondary actions (practice).
  - `button button--ghost button--compact` for canvas-level cancel controls.
- Task layout primitives:
  - `task-card task-card--instructions` for the instruction + CTA sections (PVT + 2-back share this structure).
  - `task-card task-card--canvas` for active run surfaces.
  - `task-card task-card--subtle` for lightweight recap cards (e.g., practice summary).
  - `metrics-grid` for presenting summary stats.
- Accordions use `details.task-instructions` with a built-in “+ / –” indicator; keep headings short and informative.
- Progress badges rely on `task-progress` and `task-progress--overlay` classes—avoid custom absolute positioning unless necessary.
- Feedback chips (`task-feedback` with `task-feedback--positive|negative`) provide consistent in-run acknowledgement; reuse these tone classes for new tasks.
- When introducing new screens, prefer the existing `task`, `task-card`, and `button` primitives to maintain coherence and reduce bespoke CSS.

---

## Localization (i18n)

Looplace ships with live, runtime language switching (currently English `en-US`, Spanish `es-ES`, and French `fr-FR`) powered by Fluent + `i18n-embed`. All translation assets live in the shared UI crate so every platform (desktop, web, mobile) stays in sync.

### Folder layout
```
ui/
  src/i18n.rs
  i18n/
    en-US/looplace-ui.ftl   (fallback / reference locale)
    es-ES/looplace-ui.ftl
    fr-FR/looplace-ui.ftl
```

### Adding a new language
1. Copy the fallback file:
   ```
   cp ui/i18n/en-US/looplace-ui.ftl ui/i18n/<lang-tag>/looplace-ui.ftl
   ```
   Use a valid BCP‑47 language tag (e.g. `de-DE`, `pt-BR`).
2. Translate only the message values (keep IDs and variable placeholders unchanged).
3. Run tests (the completeness test will fail if:
   - Any fallback key is missing in the new locale.
   - There are syntax errors in the FTL file).
4. Launch the app; the new tag should appear automatically in the language selector.

### Using translations in code
Call `ui::i18n::init()` once near the root (already done in each platform `App`).  
Then inside components use the short macro:
```rust
{crate::t!("nav-home")}
{crate::t!("pvt-how-step-target", trials = total_target)}
```
The `t!` macro expands to a compile‑time checked `fl!` call against the shared loader. Missing keys cause a build error in the fallback locale and a runtime warning in non‑fallback locales.

### Live switching
- The global language code is stored in a Dioxus `Signal<String>` provided via context.
- The routed subtree is keyed by that language code to force a clean remount.
- Some deeper subsections (task instruction accordions, results panels) also subscribe with a hidden “marker” `div` to ensure they re-render even if internal memoization changes.

### Best practices
- Prefer short, stable message IDs: `results-empty`, `pvt-start`, `nback-start-main`.
- Reuse shared prefixes (`pvt-`, `nback-`, `results-`, `nav-`, `home-`) for discoverability.
- For dynamic variables, pass them explicitly: `t!("pvt-how-step-target", trials = total_target)`.
- Avoid concatenating translated fragments manually—add a dedicated full sentence key instead.
- Keep punctuation inside the translation value (so locales can adapt, e.g., spacing around colons).

### Pitfalls to avoid
| Pitfall | Fix |
| ------- | --- |
| Reusing an ID for a different meaning | Create a new ID (IDs are semantic, not just placeholders). |
| Hard‑coding English in task UIs “temporarily” | Add the key now; fallback stays English anyway. |
| Building sentences via `format!("{t1} {value} {t2}")` | Create one message with a `{value}` placeholder. |
| Forgetting to subscribe to language signal in a long‑lived component | Add a hidden marker `div { "{lang_signal()}" }` or rely on the keyed subtree higher up. |

### Testing & CI
- The completeness test in `ui/src/tests/i18n_completeness.rs` parses all `t!("...")` usages and ensures:
  - Every referenced key exists in the fallback locale.
  - All other locales contain every fallback key.
- Add your new locale before enabling CI gating to avoid noise.

### Removing a key
1. Update all call sites (search for `t!("the-old-key")`).
2. Remove the key from every `.ftl` file.
3. Run tests (they should pass—no hidden usages).
4. Commit as a single “i18n: remove obsolete key <id>” change.

---

## Need help?

If something feels unclear or brittle, add a note in `AGENTS.md` under “Parking lot” or open a discussion. We’re building this to support real people—tight feedback loops keep it trustworthy.

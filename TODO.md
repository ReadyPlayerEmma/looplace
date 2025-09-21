# TODO — Looplace Frontend (dx-aligned, Web & Desktop only)

> **Scope:** Front-end demo only. No backend calls. We will run tests, compute metrics **locally**, visualize results, and support export/share (JSON/CSV/PNG).
> **Decisions:**
> • **CSR only** (no SSR).
> • **Routes live in platform crates** (`web/`, `desktop/`) exactly like the dx template.
> • **Views, engines, metrics, charts live in `ui/`** and are imported by platforms.
> • Keep `api/` and the `Echo` demo around, but do **not route** to it for the demo (or show a disabled note).

---

## M0 — Workspace alignment (match dx structure)

**Goal:** Keep the dx template shape while centralizing logic in `ui/`.

- [x] Create shared view wrappers in `ui/src/views/`:
  - `home.rs`, `pvt.rs`, `nback.rs`, `results.rs`
  - Each wraps the corresponding component (`<Home/>`, `<Pvt/>`, `<NBack2/>`, `<Results/>`).
- [x] Add `ui/src/tasks/` and `ui/src/core/` scaffolds:
  - `tasks/pvt/{engine.rs,metrics.rs,view.rs}`
  - `tasks/nback/{engine.rs,metrics.rs,view.rs}`
  - `core/{timing.rs,storage.rs,qc.rs,format.rs,platform.rs}`
- [x] Re-export a clean surface from `ui/src/lib.rs`:
  ```rust
  pub mod views { pub use crate::views::{Home, Pvt, NBack2, Results}; }
  pub mod components { pub use crate::navbar::Navbar; }
````

* [x] **Web routes** (`web/src/main.rs`): keep the template’s local `Route` and point to `ui` views:

  * `"/"` → `ui::views::Home`
  * `"/test/pvt"` → `ui::views::Pvt`
  * `"/test/nback"` → `ui::views::NBack2`
  * `"/results"` → `ui::views::Results`
* [x] **Desktop routes** (`desktop/src/main.rs`): same as web (own `Route` enum, import `ui` views).
* [ ] Keep platform-specific assets in each platform; shared CSS/images remain in `ui/assets/`.
* [x] Optionally remove/rename the sample `blog.rs` routes, or keep them as a dx tutorial page.

**Acceptance (M0):**

* Web and Desktop compile and render the **Home** page using `ui::Navbar` and `ui::views::Home`.
* No server calls are made.

---

## M1 — PVT MVP (engine → metrics → local summary)

* [x] **Engine (`ui/tasks/pvt/engine.rs`)**

  * RAF scheduler + `performance.now()`
  * ITI jitter 2–10 s
  * Keydown/tap input; **anticipation (false start)** detection
  * [ ] Pause/Resume controls (abort wired)
* [x] **Metrics (`ui/tasks/pvt/metrics.rs`)**

  * `median_rt_ms`, `mean_rt_ms`, `sd_rt_ms`, `p10_rt_ms`, `p90_rt_ms`
  * `lapses_ge_500ms`, `minor_lapses_355_499ms`, `false_starts`
  * `time_on_task_slope_ms_per_min`
* [x] **QC flags (`ui/core/qc.rs`)**

  * Visibility/tab blur counters; min-trial guard; device snapshot
* [x] **Persist (`ui/core/storage.rs`)**

  * Append a single **summary JSON** per run to `localStorage` key `looplace_summaries`
* [x] **View (`ui/tasks/pvt/view.rs` + `ui/views/pvt.rs`)**

  * Minimal test UI, instructions, big key/tap target
  * End-of-run stat card preview

**Acceptance (M1):**

* Complete a 3-minute PVT on web and desktop; summary saved locally.

---

## M2 — Results UI (lists + charts + export)

* [ ] **Results list (`ui/results/list.rs`)**

  * Reverse-chronological runs; columns: date/time, platform, median RT, lapses, QC flag
  * Filter by task; search by date range (P1)
* [ ] **Details (`ui/results/detail.rs`)**

  * Stat cards + notes + QC chips
* [ ] **Charts (`ui/results/charts.rs`)**

  * **SVG sparkline** of median RT across sessions
  * **SVG bars**: lapses (≥500 ms) and false starts per session
  * Optional histogram of RT bins (100 ms) (P1)
* [ ] **Export (`ui/results/export.rs`)**

  * JSON: copy to clipboard + download
  * CSV: selected fields (task, when, median, lapses, false starts, slope, qc)
  * PNG: render results panel to image (DOM→canvas→data URL)

**Acceptance (M2):**

* After 3+ runs, Results shows a list, a detail view with charts, and working JSON/CSV/PNG exports.

---

## M3 — 2-Back MVP (2-back only)

* [ ] **Engine**: balanced letter stream with seed; record RTs
* [ ] **Metrics**: hits, misses, false alarms, correct rejections; **d′** and criterion; accuracy%; RT stats
* [ ] **Persist**: summary JSON to localStorage
* [ ] **View**: instructions tooltip (+ 30–45 s practice block)

**Acceptance (M3):**

* Run a 2-back, see d′ on the result detail and in trend charts.

---

## M4 — UX, a11y, desktop polish

* [ ] High-contrast theme; keyboard-only operation verified
* [ ] Respect reduced-motion preference (chart transitions)
* [ ] Desktop: window min size/title; (P1) Save dialogs for PNG/CSV/JSON

---

## Data model (localStorage summary)

Key: `looplace_summaries` → JSON array

```json
{
  "id": "pvt-2025-09-07T17:03:20Z-uuid",
  "task": "pvt|nback2",
  "created_at": "2025-09-07T17:03:20Z",
  "client": { "platform": "web|desktop", "tz": "America/Chicago" },
  "metrics": { ...task-specific fields... },
  "qc": { "visibility_blur": 0, "min_trials_met": true, "notes": "" },
  "notes": ""
}
```

---

## File-by-file checklist

**ui/**

* [x] `src/core/{timing.rs,storage.rs,qc.rs,format.rs,platform.rs}`
* [x] `src/tasks/pvt/{engine.rs,metrics.rs,view.rs}`
* [x] `src/tasks/nback/{engine.rs,metrics.rs,view.rs}`
* [x] `src/results/{list.rs,detail.rs,charts.rs,export.rs}`
* [x] `src/views/{home.rs,pvt.rs,nback.rs,results.rs}`
* [x] `src/lib.rs` (re-exports `views`, `components`)

**web/**

* [ ] `src/main.rs` — own `Route` enum → import `ui::views::*`
* [ ] `assets/*` — keep; add favicon/theme as needed

**desktop/**

* [ ] `src/main.rs` — own `Route` enum → import `ui::views::*`
* [ ] `assets/*` — keep; add window css if needed

**api/**

* [ ] Leave as-is; do not route to Echo in demo (or show disabled notice)

---

## Dev overlay (optional P1)

Toggle via `?debug`: last RTs, RAF delta, visibility count, seed, platform info.

---

## Acceptance criteria (demo complete)

* Web & Desktop can run **PVT** end-to-end and see results with charts.
* Results list shows multiple runs; detail view exports JSON/CSV/PNG.
* No network calls; all data local.
* Keyboard-only works; high-contrast passes AA on results.

---

## Test matrix (manual)

| Area                   | Web | Desktop |
| ---------------------- | --- | ------- |
| PVT run start→finish   | ☐   | ☐       |
| Anticipation detection | ☐   | ☐       |
| Lapse counting         | ☐   | ☐       |
| Pause/Abort            | ☐   | ☐       |
| Visibility QC          | ☐   | ☐       |
| Save/load summaries    | ☐   | ☐       |
| Charts render          | ☐   | ☐       |
| Export JSON/CSV/PNG    | ☐   | ☐       |
| Keyboard-only nav      | ☐   | ☐       |
| High-contrast          | ☐   | ☐       |

---

## Notes

* Keep heavy DOM work out of the critical test loop; prefer light DOM during runs.
* Use pure functions for metrics to enable unit tests later.
* Avoid storing raw trial streams by default (privacy & portability).
* When we add backend later, we’ll just wire `save_summary`/`get_summaries` server functions without touching the views.
* Log papercuts (focus drift, layering bugs, etc.) while they’re fresh so they don’t stack up and sap the fun from the project.
* CI now covers macOS Apple Silicon bundles and Windows x64 zips; expand to Linux or additional Windows packaging when needed.

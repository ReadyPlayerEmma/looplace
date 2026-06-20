# Looplace
[![Version](https://img.shields.io/badge/version-0.2.0-orange.svg)](https://github.com/ReadyPlayerEmma/looplace/releases) [![License: MPL 2.0](https://img.shields.io/badge/License-MPL_2.0-brightgreen.svg)](./LICENSE) [![Localization](https://img.shields.io/badge/i18n-en--US%20|%20es--ES%20|%20fr--FR-blue)](#localization) [![Built with Dioxus](https://img.shields.io/badge/built%20with-dioxus-6A5ACD.svg)](https://dioxuslabs.com)

Small loops • clear minds.
Looplace is a cross-platform, **local-first app for understanding your own body and mind**. It began as a cognitive testing tool — a Psychomotor Vigilance Task and a 2-back working-memory task — and is growing into an open, fully-local **health-data platform**, starting with blood glucose from the **FreeStyle Libre 2**. Collect your own signals, watch them over time, and explore how they relate — all stored locally on your device, under your control. Built with **Rust** and **Dioxus**, desktop-first (macOS/Windows/Linux), with shared web and mobile shells for contributors.

> **Not medical advice.** Looplace is a tool for exploring and understanding your *own* data, with local autonomy — not a diagnostic device. Clinical interpretation belongs with a clinician alongside standard labs.

---

## Why we’re building this

Looplace started as a way to help close friends track cognition with compassion. My friend Tom is navigating Alzheimer’s treatments and needed a simple, repeatable way to see how interventions shape his attention and working memory. At the same time, I am rebuilding my own focus after a prolonged illness, and another friend flan wants to spot trends alongside lifestyle shifts. The shared need to observe changes over time—without clinical overhead—inspired this project.

Our goal is to keep the tools approachable while layering in richer analysis: journaling life changes, correlating treatments, and giving people agency over their own data. Increasingly that means more than cognition — glucose today, with activity and sleep to follow — so you can gather your signals in one local place, see them on a shared timeline, and notice the patterns (does what you eat shape your focus?). If you are exploring similar journeys, we hope Looplace feels like a caring companion.

---

## Key features

**Cognition**
- **Psychomotor Vigilance Task (PVT)** with precise timing, lapse tracking, and local summaries.
- **2-back working-memory task** with a short practice block, d′/criterion metrics, and immediate response feedback.

**Health**
- **Blood glucose (FreeStyle Libre 2)** via a native-Rust USB driver — sync your reader in one click and see your latest value, a trend sparkline with scan / food (🍎) / exercise (🏃) markers, and a recent-readings list. The driver is **read-only** against the reader, and nothing leaves your machine.

**Platform**
- **Local-first storage**: a unified on-device store (Parquet) holds cognition sessions *and* health observations on one timeline, ready for export and correlation. Existing JSON summaries migrate automatically — your original is backed up first.
- **Live localization**: English, Spanish, and French with instant in-app language switching (no reload) and compile-time-checked translation keys.
- **Shared UI crate** so additions land across desktop, web, and mobile from one codebase.

---

## Platforms

- **Desktop (focus)**: macOS, Windows, and Linux via the `desktop/` launcher. Glucose sync is desktop-only — it talks to the reader over USB.
- Web SPA and mobile shells remain available for contributors; on those targets the health features show a desktop-only note. (Cloudflare Pages deploys are currently paused.)
- **Embedded assets (desktop)**: all core UI CSS is embedded in the binary (no external stylesheet files required on Windows). This keeps the portable Windows zip minimal and reduces the chance of missing-asset issues.

---

## Quick start

End users who just want to try Looplace can download the latest builds from the [GitHub Releases page](https://github.com/ReadyPlayerEmma/looplace/releases).

Developers can run the desktop app locally with:

```bash
cargo run -p looplace-desktop --features desktop
```

(Or `dx serve` from inside `desktop/` if you have the matching `dioxus-cli` installed — note older CLIs don’t accept `--platform`.)

For additional targets, workspace layout, accessibility/timing guidelines, and deployment notes, see [CONTRIBUTING.md](./CONTRIBUTING.md). Design choices and the shared component system are documented under “UI design system” in that guide.

---

## Roadmap & docs

- [TODO.md](./TODO.md) tracks milestones (cognition tasks shipped; the health/glucose vertical is in progress).
- [AGENTS.md](./AGENTS.md) captures project context, approvals, and the parking lot.
- [CONTRIBUTING.md](./CONTRIBUTING.md) stores developer guidance, module maps, and infrastructure tips.

---

## License

Looplace is released under the [Mozilla Public License 2.0](./LICENSE). The optional FreeStyle Libre 2 protocol key constants live in a separate crate (`looplace-libre-keys`) behind a feature flag and are excluded from default builds.

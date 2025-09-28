# Looplace
[![Version](https://img.shields.io/badge/version-0.1.7-orange.svg)](https://github.com/ReadyPlayerEmma/looplace/releases) [![License: MPL 2.0](https://img.shields.io/badge/License-MPL_2.0-brightgreen.svg)](./LICENSE) [![Localization](https://img.shields.io/badge/i18n-en--US%20|%20es--ES%20|%20fr--FR-blue)](#localization) [![Built with Dioxus](https://img.shields.io/badge/built%20with-dioxus-6A5ACD.svg)](https://dioxuslabs.com)

Small loops • clear minds.
Looplace is a cross-platform cognitive testing app built with **Rust** and **Dioxus**. Today we prioritise a desktop-first experience (macOS/Windows) while keeping the shared web and mobile shells available for contributors. Back-end work will land as **Dioxus server functions** (Axum under the hood), with an eventual Cloudflare Workers + D1 deployment path.

---

## Why we’re building this

Looplace started as a way to help close friends track cognition with compassion. My friend Tom is navigating Alzheimer’s treatments and needed a simple, repeatable way to see how interventions shape his attention and working memory. At the same time, I am rebuilding my own focus after a prolonged illness, and another friend flan wants to spot trends alongside lifestyle shifts. The shared need to observe changes over time—without clinical overhead—inspired this project.

Our goal is to keep the tools approachable while layering in richer analysis: journaling life changes, correlating treatments, and giving people agency over their own data. If you are exploring similar journeys, we hope Looplace feels like a caring companion.

---

## Key features

- **Psychomotor Vigilance Task (PVT)** with precise timing, lapse tracking, and local summaries.
- **2-back working-memory task** with a short practice block, d′/criterion metrics, and immediate response feedback.
- **Local-first storage**: every run produces a lightweight JSON summary ready for export or deeper analysis.
- **Live localization**: English, Spanish, and French with instant in-app language switching (no reload) and compile-time checked translation keys.
- **Shared UI crate** so additions land across desktop, web, and mobile with minimal effort.
- **Unified task design system**: both tasks share consistent task cards, primary/secondary buttons, expandable instructions, and at-a-glance metrics.

---

## Platforms

- **Desktop (focus)**: macOS and Windows via the `desktop/` launcher.
- Web SPA and mobile shells remain available for contributors; Cloudflare Pages deploys are currently paused.

---

## Quick start

End users who just want to try Looplace can download the latest builds from the [GitHub Releases page](https://github.com/ReadyPlayerEmma/looplace/releases).

Developers can spin up the desktop experience locally with:

```bash
cargo install dioxus-cli
cd desktop
dx serve --platform desktop
```

For additional targets, workspace layout, accessibility/timing guidelines, and deployment notes, see [CONTRIBUTING.md](./CONTRIBUTING.md).

Design choices and the shared component system are documented under “UI design system” in that guide.

---

## Roadmap & docs

- [TODO.md](./TODO.md) tracks milestones (currently M2 — Results UI).
- [AGENTS.md](./AGENTS.md) captures project context, approvals, and the parking lot.
- [CONTRIBUTING.md](./CONTRIBUTING.md) stores developer guidance, module maps, and infrastructure tips.

---

## License

Looplace is released under the [Mozilla Public License 2.0](./LICENSE).

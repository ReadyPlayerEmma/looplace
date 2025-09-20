# Agent Notes

## Project context
- Looplace is a Dioxus-generated workspace; treat it as the canonical layout and follow existing module boundaries.
- UI logic belongs in `ui/`; platform crates (`web/`, `desktop/`, `mobile/`) stay thin wrappers.
- Server functions (`api/`) remain stubbed during the front-end demo.

## Working guidelines
- Assume latest Rust, Dioxus, and dx CLI behaviour—cross-check docs when something feels unfamiliar or mismatched with prior knowledge.
- Prefer existing architectural patterns before introducing new abstractions; align with TODO milestones.
- Keep code platform-agnostic inside `ui/`; isolate platform glue to respective crates.
- Verify timing-sensitive code uses `request_animation_frame`/`performance.now()` wrappers from Dioxus when available.
- Whenever UI appearance or behaviour might change, ask the user to run a quick real-world test for validation.

## Research + questions
- If APIs differ from memory, consult current Dioxus documentation or ask the user before diverging from template conventions.
- Track notable API changes, workarounds, or quirks here for future reference.

## Parking lot
- `TODO` items to revisit or clarify: (add entries as they surface).
- Potential documentation gaps or missing context to flag with the user.

# GUI Graph Implementation Pickup

Updated: 2026-09-28

## What is implemented in the current worktree

- Native `egui`/`eframe` graph editor with selection, scrollable inspector,
  task creation, graph panning/zooming, arrow-key navigation, and persistent
  machine-local dragged-card positions.
- Separate structural-parent, planned `depends_on`, and active `blocked_by`
  relationships. Inverse views are derived rather than stored twice.
- Relationship overlays: green parent arrows, blue dependency arrows, and red
  active-blocker arrows. Tags render as nested color-constellation regions.
- Tag/search/filter controls, Today date shortcuts, citations with explicit
  Open controls, notes, and a Focus Window frame around active work.
- Event-backed focus sequences: `todo focus create|add|ls` and inspector
  controls append existing tasks in cognitive order without changing readiness.
- Event-backed sessions: `todo session start|stop|ls` and inspector Start/Stop
  controls. One active session per task is enforced.
- Event-backed scalar `ItemPatch` for raw JSON: title, priority, state, due,
  defer, and scheduled dates validate then apply in one transaction.
  Relationship and note changes deliberately remain explicit.

## Current verification state

The v9 protocol fixture is now corrected. The latest full test run passes all
workspace tests and formatting checks:

```sh
./dev fmt
./dev test
git diff --check
```

The v9 raw-patch projection and GUI parser compile. The store regression test
covers successful multi-field application and rejection without partial writes.

## Remaining work for the active "implement 1-7" goal

1. Add a timeline view using `scheduled_on`; do not overload due or defer: due
   is a commitment and defer controls readiness.
2. Improve Focus Window layout from a frame to done-above, active-center,
   future-below, with selected focus-sequence neighbors.
3. Keep cross-project `contributes_to` / `implements` as future typed edges;
   do not model them as a second structural parent.
4. Add an inspector display-settings tab if still desired, so relationship
   sections can be individually hidden without affecting graph data.

## Protocol sequence

- v6: active blocker events.
- v7: focus sequence events.
- v8: session events.
- v9: atomic scalar item patch event.

Each protocol document is in `docs/protocols/`. New ledger writes use the
latest protocol; import accepts historical v5 through v9 events.

## Future tag color-wheel design

The current constellation uses a deterministic hash color per visible prefix.
The desired replacement is a hierarchy-aware color wheel:

1. Count visible top-level tag groups and divide the hue wheel into that many
   evenly spaced sectors.
2. Assign each top-level group one sector, with deterministic alphabetical
   ordering.
3. For children, divide the parent sector by visible sibling count. The first
   or largest sibling stays closest to the parent hue; later/smaller siblings
   receive progressively greater hue or lightness deviation, with the final
   sibling furthest from the base color.
4. Reuse the mapping for region fills, outlines, and tag chips.

Implement this as a pure deterministic `TagPalette` over the visible tag tree;
never derive colors from database IDs or random state. Add tests for zero, one,
and many sibling groups.

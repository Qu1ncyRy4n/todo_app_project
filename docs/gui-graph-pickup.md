# GUI Graph Implementation Pickup

Updated: 2026-09-28

## What is implemented in the current worktree

- Native `egui`/`eframe` graph editor with selection, scrollable inspector,
  task creation, graph panning/zooming, arrow-key navigation, and persistent
  machine-local dragged-card positions.
- Separate structural-parent, planned `depends_on`, and active `blocked_by`
  relationships. Inverse views are derived rather than stored twice.
- Relationship overlays: green parent arrows, blue dependency arrows, red
  active-blocker arrows, and dashed typed-relation arrows, all with fixed-size
  arrowheads. Tags render as nested organic hull regions.
- Layout commands: Normalize, Focus layout, Timeline, and Separate tags, each
  undoable through Undo layout.
- Reversible deletion with confirmation, plus a Deleted list with Restore.
- Tag/search/filter controls, Today date shortcuts, calendar pickers, citations
  with explicit Open controls, notes, and a Focus Window around active work.
- Event-backed focus sequences: `todo focus create|add|ls` and inspector
  controls append existing tasks in cognitive order without changing readiness.
- Event-backed sessions: `todo session start|stop|ls` and inspector Start/Stop
  controls. One active session per task is enforced.
- Event-backed scalar `ItemPatch` for raw JSON: title, priority, state, due,
  defer, and scheduled dates validate then apply in one transaction.
  Relationship and note changes deliberately remain explicit.

## Completed goal: slices 1-7 and the follow-on slices 8-17

The original seven-slice goal is complete, followed by a second batch drawn from
dogfooding notes. Every slice below landed as its own commit with tests.

1. Planned `depends_on` split from active `blocked_by`, inverse views derived.
2. Hierarchical tag coloring: the hue wheel is divided between visible top-level
   groups, each group's sector subdivided between its visible children, and depth
   darkens. The previous hash mapping remains as the `group_hash` mode.
3. Ordered focus sequences over existing tasks.
4. Focus Window layout: completed above, active centered, future below, bands
   centered on one axis and wrapping away from the center.
5. Time views: scheduled dates, Today shortcuts, sessions, links, a scheduled-date
   timeline with a day ruler, and a calendar picker on every date field.
6. Atomic validated scalar patches and persistent local node layout.
7. Typed cross-project `contributes-to` and `implements` relations.
8. Tag color exclusion: a node's accent comes from its own deepest tag, never from
   a region it merely sits inside.
9. Arrowheads keep one size at any zoom and any edge length.
10. Normalize: layered longest-path layout flowing down or right.
11. Organic tag regions: smoothed convex hulls instead of bounding rectangles.
12. Configurable presentation through `theme.toml`, including node shape.
13. Parent containment regions around a parent and its whole subtree.
14. Separation pass pushing nodes out of foreign tag regions.
15. Reversible deletion behind a confirmation, with restore.
16. Task weight from priority, urgency, difficulty, estimate, and subtree size.
17. Individually hideable inspector sections.
18. Inspector grouped into Status, Relationships, Details, Schedule, and Advanced,
    each collapsible. The editor no longer deletes tasks.
19. One filter sidebar; the toolbar carries actions only.
20. Hierarchical tri-state tag filter: neutral, only, hidden, with inheritance.
21. The timeline ruler retires with its layout.
22. Tag regions drawn per cluster of nearby members, with labels above the region
    and clipped to its width.
23. Parent tasks draw as titled boxes holding their subtree.
24. Overlapping tasks are pushed apart after every layout, and on demand.
25. Focus layout exiles and dims non-active work, then frames the active band.
    An Active work only filter hides it entirely instead.
26. Nodes glide to new positions instead of jumping.
27. The day ruler is pinned to the canvas, so no pan can hide it.
28. Tasks are named by title and by proquint, never by raw hexadecimal.
29. Create child from the inspector.
30. Titles wrap and the card grows to hold them.
31. The inspector and filter panels resize.
32. Tasks inherit the tags of the tasks above them, configurably.
33. A task holding active work reads as active, derived and never stored.
34. Tag colors come from a bright rainbow ramp; borders carry the tag family.
35. Each visual channel is told what it means, with example themes.
36. A parent task is the box its subtree sits in.
37. Normalize hangs subtrees under their head, finished prerequisites above.
38. Normalize pulls active heads together, each carrying its subtree.
39. Group into columns by tag, sequence, time, priority, or state, and sort
    within them.
40. The timeline has an orientation, a scale, and duration blocks.
41. A void state for items created in error, distinct from canceled.
42. Dragging a container follows the pointer instead of accelerating away, and
    arrows are aimed at the rectangles cards actually occupy.
43. View and force settings sit above the filters, with the forces as live
    sliders that can be written back to `theme.toml`. A hierarchy grows down or
    right, and dragging empty canvas moves the view.

## Configuration

Presentation lives in `theme.toml` beside `config.toml` in the config directory.
A missing file keeps the defaults; a malformed file or an unknown key is reported
rather than ignored. Reload theme re-reads it in place, and writes the current
defaults the first time as an editable starting point.

Sections: `[node]`, `[edge]`, `[tag]`, `[layout]`, `[focus]`, `[timeline]`,
`[parent]`, `[weight]`, `[inspector]`. Example arrangements live in
`docs/themes/`, including the visual channels: a node's bar, body, and border are
each told what they stand for rather than being wired to one meaning. A `[safety]` section from an earlier
version is still accepted so existing files keep loading, but it no longer does
anything: deleting left the editor.

Values that carry data or protocol meaning are deliberately not configurable.

## Current verification state

```sh
./dev fmt
./dev test
git diff --check
```

All workspace tests pass, including protocol round trips, view rebuilds from the
log, and the CLI integration suite.

## Deferred by decision

These were considered and put off deliberately, not overlooked.

0. A scheduled time of day. The ledger stores a scheduled date only, so the
   timeline's second, hourly dimension has nothing to place tasks against. It
   needs a field and a protocol version, and is the next thing the timeline
   wants.
1. A forced purge, behind a confirmation that requires retyping the task title.
   Voiding and deleting both stay reversible until then.
2. The proquints and todo-standard skill, pending which standard is meant.
   TE-0012 records what shipped and what the rendering deliberately does not do.
3. Live physics: continuous gravity and repulsion, so the graph self-organizes and
   reacts while dragging. The eased transitions and the overlap pass are the
   settling half of it; the always-running simulation is the part left.
4. Timeline snapping: dragging a task onto a day to set its scheduled date.
5. Grouping and sorting in the CLI listings, which so far exist only in the
   graph editor.

## Remaining work

1. Order nodes within a normalized layer by predecessor barycenter to cut edge
   crossings. Today the order is alphabetical and deterministic.
2. Approximate tag regions by their drawn hull during separation, rather than by
   their centroid. Clustering has narrowed the regions, so this matters less.
3. Break cycles deliberately in the layered layout instead of parking cyclic nodes
   in a trailing layer.
4. Decide whether the spore-galaxy reading wants more than the `circle` node shape
   already available, such as size-by-weight tuned against real data.
5. Measure region label widths with a text galley rather than estimating from the
   font size, if a clipped label ever lands visibly short or long.
6. Compacting active heads runs after the overlap pass, so branches keep their
   spacing while heads move; a head's own branch can end up overlapping a
   neighbor's. Re-running Unstack separates them.
7. Tag inheritance walks the ancestor chain per task on each read. Fine at this
   size; an index would be the upgrade if a tree ever gets deep and wide.

## Protocol sequence

- v6: active blocker events.
- v7: focus sequence events.
- v8: session events.
- v9: atomic scalar item patch event.
- v10: reversible item deletion (`item.deleted`, `item.restored`).
- v11: difficulty and effort estimate fields.
- v12: typed cross-project relations.
- v13: the void state, for items created in error.

Each protocol document is in `docs/protocols/`. New ledger writes use the latest
protocol; import accepts historical v5 through v13 events. Deletion is a
projection state, not ledger truth: the log stays append-only and no historical
event is removed or rewritten.

## Weight, and what it is not

Weight is computed in the presentation layer from priority, days until due,
difficulty, estimate, and visible descendant count, as a weighted mean of
normalized inputs. It stays between 0 and 1 whatever the coefficients are, a
missing input contributes nothing rather than reading as low weight, and a
coefficient of zero removes its input. Nothing in the ledger derives readiness,
ordering, or state from it, so the weighting can change without a protocol change.

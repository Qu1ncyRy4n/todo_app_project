# GUI Graph Implementation Pickup

Updated: 2026-09-30

The stable names and visual contracts for family frames, anchors, tree lines,
tag clouds, and the planned Appearance Lab live in
[Graph Design Language](gui-graph-design-language.md). Read it before changing
graph rendering; this pickup focuses on implementation state.

## What is implemented in the current worktree

- Native `egui`/`eframe` graph editor with selection, scrollable inspector,
  task creation, graph panning/zooming, arrow-key navigation, and persistent
  machine-local dragged-card positions.
- Separate structural-parent, planned `depends_on`, and active `blocked_by`
  relationships. Inverse views are derived rather than stored twice.
- Relationship overlays: green parent arrows, blue dependency arrows, red
  active-blocker arrows, and dashed typed-relation arrows, all with fixed-size
  arrowheads. Tags render as smooth per-task clouds: same-tag circles join with
  short soft tubes, while distant matching tasks remain separate. Each card has
  one cloud from its deepest effective tag (or a quiet neutral cloud when
  untagged), so colors do not compound. The older hull is still a theme option.
- Layout commands: Normalize, Focus layout, Timeline, and Separate tags, each
  undoable through Undo layout.
- Tag filter tri-states, Today date shortcuts, calendar pickers, citations with
  explicit Open controls, notes, and a Focus Window around active work.
- Search across titles, both identifier renderings, and effective tags. `/`
  reaches the box from the canvas, Escape clears it, Enter selects the first
  match, and non-matching cards dim rather than vanish so edges survive a query.
- Both sidebars collapse from the toolbar.
- Event-backed focus sequences: `todo focus create|add|ls` and inspector
  controls append existing tasks in cognitive order without changing readiness.
- Event-backed sessions: `todo session start|stop|ls` and inspector Start/Stop
  controls. One active session per task is enforced.
- Event-backed scalar `ItemPatch` for raw JSON: title, priority, state, due,
  defer, and scheduled dates validate then apply in one transaction.
  Relationship and note changes deliberately remain explicit.

Deletion is deliberately absent from the editor. `todo rm --force` and
`todo restore` remain in the CLI, and protocol v10 still carries the events.

## Current graph revision

- **Reset filters** restores every transient visibility and query control to the
  opening view. It deliberately leaves the arrangement, viewport, live theme,
  and `theme.toml` untouched.
- Hierarchy sorts roots and siblings by the selected order (after giving active
  roots precedence), and does not run generic overlap repulsion afterward. The
  tree remains the source of horizontal position, while parent-to-child and
  planned prerequisite-to-dependent edges jointly determine vertical rows. This
  keeps dependency arrows flowing down without flattening family structure.
  **Unstack** is available for exceptional wrapped-card collisions.
- Parent containment boxes wrap nearby descendants and become bounded
  continuation boards when a child is distant; the parent card stays at its
  tree-assigned head position. Every tag cloud anchors
  to the measured card center, including parent cards, so no cloud is left at a
  former layout position.
- Time, priority, and state grouping columns use semantic scan order rather than
  alphabetical labels: overdue through undated, critical through low, and active
  through void.
- The default Group arrangement is **Hierarchy**. The measured parent card is
  the attached tab of a family body; its descendants sit in an unpainted child
  space so the graph and tag clouds remain visible there. A child that has its
  own descendants repeats that silhouette inside the parent's space. The frame
  itself uses an opaque fill, preventing overlap colors from compounding. Bent
  lines ending in open circles distinguish family links from dependency arrows.
  View settings expose header/attached-tab, recessed-board/nested-family, and
  transparent/dark child-space choices. Attached tab, nested family, and
  transparent child space are the defaults. The fixed design demos have been
  removed from the inspector.
- Child depth is configurable from one to six generations or All; the default
  is one. A collapsed parent shows an expand badge, while active and selected
  paths remain visible. Distant child boards are capped relative to the parent
  card, with a continuation control to open the complete span. A short stalk
  attaches each parent card to its own child board.
- The collapsed-family continuation is a small `…` button centered on the
  parent card's child-side edge, where its tree would continue, rather than a
  corner badge. It appears only while children are hidden and reveals them on
  click. Right-clicking a card offers **Family display → Box / Tree**;
  that global display choice is intentionally not a persistent inspector control.
- G over empty graph canvas normalizes the visible graph. G over a card, or its
  context-menu command, normalizes only that connected parent/child tree and
  leaves unrelated families in place. Focus now separates wrapped active rows,
  places active-holding parents with them, and omits spanning family fills.
- Cloud fills are pre-blended once against the active canvas and drawn opaque, so
  a same-tag circle and its connecting tube remain one color at every overlap.
- Appearance is a dedicated inspector section. It owns card silhouette and
  wrapping, tag-cloud treatment, family treatment, and motion/arrow presentation;
  View remains for hierarchy depth and arrangement behavior. Cloud members may be
  the original circles or optional card-sized rounded rectangles joined by rails.
  The latter keeps a tag visibly attached to the cards it annotates without
  turning it into a parent board. Card clouds use each card's measured bounds,
  including wrapped-title height, and cloud labels sit inside their own bounds.
- Scroll zoom consumes raw wheel and pinch events, rather than egui's deliberately
  inertial smoothed scroll state. The neutral pinch scale no longer produces a
  zoom step on every idle frame. Family paint order is structural (ancestor before
  descendant), avoiding border flicker when nearly equal animated boards swap
  area order. Same-tag child outlines can soften within their family well; selected
  and differently tagged cards keep their contrast. Appearance can also hide a
  redundant same-family outline completely.

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
23. Parent tasks draw as a head card inside a containment box holding their subtree.
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
36. A parent task remains the visible head of the box its subtree sits in.
37. Normalize hangs subtrees under their head, finished prerequisites above.
38. Normalize packs complete task families into compact rows, preserving each
    subtree's internal shape and placing dependency-linked families in ordered
    bands. The arrangement is centered in the graph viewport.
39. Group into columns by tag, sequence, time, priority, or state, and sort
    within them. Semantic columns scan time, priority, and state in working
    order rather than alphabetical label order.
40. The timeline has an orientation, a scale, and duration blocks.
41. A void state for items created in error, distinct from canceled.
42. Dragging a container follows the pointer instead of accelerating away, and
    arrows are aimed at the rectangles cards actually occupy.
43. View and force settings sit above the filters, with the forces as live
    sliders that can be written back to `theme.toml`. A hierarchy grows down or
    right, and dragging empty canvas moves the view.
44. Group by three ordered keys, with the sort order governing placement in
    every arrangement.
45. Search across titles, both identifier renderings, and effective tags. `/`
    reaches the box from the canvas, Escape clears it, Enter selects the first
    match, and non-matching cards dim rather than disappear so edges survive the
    query. Both sidebars toggle off from the toolbar.
46. Adaptive family display, per TE-0015. A family box contains its children when
    they fit nearby and becomes a bounded continuation board when one is distant,
    so a far descendant no longer stretches a dark span across the canvas. A short
    stalk attaches the parent card to its own board, which reads as ownership even
    when the card sits among its children.
47. Configurable child depth from one to six generations, or All, defaulting to
    one. Collapse is a view layer only: the paths to active and selected work stay
    revealed, and any parent can be expanded past the global cap from its badge.
48. Focus separates its own bands. Wrapped active rows no longer share one Y,
    non-focus cards are placed outside the measured active span, and no family
    fill is painted across that boundary. One `focus_is_active` classification now
    drives placement, framing, and dimming together, so the gold frame and the
    central band cannot disagree.
49. G normalizes. Over empty canvas it arranges the whole visible graph; over a
    card, or from its context menu, it arranges only that connected family and
    leaves other families where they are. Both are undoable, and Undo restores
    expansion state along with positions.
50. A family band is one padding thick on every side. The body grows from the
    well alone, and the parent card is folded in only when it lies within one
    card height of the frame, so a distant card can no longer turn the frame
    into a slab spanning the gap to its children.
51. One family reading can be forced across the whole graph: Adaptive, Boxed
    (every family framed, never capped), or Connected (no frames, links only).
    Family links are gated on the parent-task overlay, so Connected keeps them.
52. An open family drops its fill, well edge, and attachment stalk, since its
    own cards carry the structure. The boundary returns as a dashed outline
    while that family is being dragged, and only then.
53. Family connectors are drawn as a casing plus a core, one stroke the shared
    blue and the other the family's own fill, so a link says which family it
    belongs to. `parent.connector_style` chooses which carries the color.

## Configuration

Presentation lives in `theme.toml` beside `config.toml` in the config directory.
A missing file keeps the defaults; a malformed file or an unknown key is reported
rather than ignored. Reload theme re-reads it in place, and writes the current
defaults the first time as an editable starting point.

Sections: `[node]`, `[edge]`, `[tag]`, `[layout]`, `[focus]`, `[timeline]`,
`[parent]`, `[weight]`, `[inspector]`. In `[parent]`, `header_style` accepts
`header` or `attached_tab`, and `child_style` accepts `recessed_board` or
`nested_family`. `well_fill` accepts `transparent` or `dark`. `family_view`
accepts `adaptive`, `boxed`, or `connected`, and `connector_style` accepts
`fill_core` or `blue_core`. In `[tag]`, `cloud_member_shape` accepts `circle`
or `card` when `region_shape = "cloud"`; the default remains `circle`. In
`[node]`, `soften_outline_in_family` defaults to `true`; optionally set
`hide_outline_in_family = true` to remove only redundant same-family outlines.
In `[parent]`,
`clip_well_below_attached_tab` defaults to `true`, keeping an attached parent
card out of its dark child-space inset. Example
arrangements live in
`docs/themes/`, including the visual channels: a node's bar, body, and border are
each told what they stand for rather than being wired to one meaning. A `[safety]` section from an earlier
version is still accepted so existing files keep loading, but it no longer does
anything: deleting left the editor.

Values that carry data or protocol meaning are deliberately not configurable.

## Current verification state

```sh
./dev check   # cargo fmt --check, then clippy --workspace --all-targets -D warnings
./dev test    # 220 tests: 150 todo-gui, 35 todo-core, 22 todo-store, 13 todo-cli
```

All workspace tests pass, including protocol round trips, view rebuilds from the
log, and the CLI integration suite.

The editor's own coverage is the pure layers only: `geometry.rs`, `theme.rs`,
`palette.rs`, `tags.rs`, and free functions lifted out of `main.rs` for the
purpose, such as `matches_query`. Nothing drives an actual window, so any change
to panel wiring, hotkeys, or focus handling has to be confirmed by running
`./dev gui`. The following have not been exercised in a running window and are
what to check there first:

- the `/` hotkey and its focus handoff;
- the `G` hotkey over a card versus over empty canvas;
- whether a family band now reads as a border at every parent-to-child distance,
  which is the TE-0015 image 9 case;
- the dashed boundary on an open family, which appears only mid-drag;
- whether the connector casing separates families legibly, and whether
  `blue_core` reads better than the `fill_core` default against a dark fill;
- the three `family_view` states against a real ledger, especially Connected on
  a dense graph;
- whether the parent-to-board stalk and the continuation chip read clearly at
  different zoom levels;
- smooth versus card-shaped clouds over a single card, a row of matching cards,
  and overlapping unrelated tags; confirm labels stay with their clouds;
- scroll-wheel and trackpad zoom on the canvas: it should stop as soon as the
  physical gesture stops, while still anchoring the hovered card;
- the compact chevron child-depth control at both a parent card and its attached
  family frame, plus same-family outline softening with and without selection;
- the TE-0015 screenshot cases: normalize a large family, expand one child,
  arrange that family from its context menu, then switch to Focus and confirm no
  inactive card sits inside the gold boundary.

Expansion state is deliberately in memory only. It is not written to
`gui-layout.json`, so a restart returns to the configured child depth. Node
offsets still persist.

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
6. Multi-user access and hosting: a sync server, a browser build, or a signed
   native distribution. Deferred until PromiseGrid reaches 1.0, because routes B
   and C would build the synchronization and ownership machinery the Grid is
   expected to supply. TE-0014 records the routes, and the two questions that
   must be settled first: whether a shared ledger is one list or many, and
   whether concurrent divergence is reported or resolved silently.

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
6. Family packing uses card clearance around each subtree's node centers, not
   the exact rendered nested-board outline. Unusually deep or wrapped families
   may still need Unstack or manual adjustment.
7. Tag inheritance walks the ancestor chain per task on each read. Fine at this
   size; an index would be the upgrade if a tree ever gets deep and wide.
8. The three appearance questions TE-0015 left open after its trial: whether the
   stalk and continuation shape read at all zooms, whether Focus should regain a
   thin family breadcrumb, and whether one generation is the right default depth
   on a large real ledger. All three need eyes on a running window, not a test.

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

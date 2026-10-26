# GUI Graph Implementation Pickup

Updated: 2026-10-26

## Session, October 14

Shipped and confirmed in a running window by the developer:

- Dark mode and a borrowed terminal palette (slice 54). `[colors]` in
  `theme.toml`; the developer's file points at
  `~/.config/ghostty/themes/green`. Another agent is wiring matugen through
  Nix to generate that file.
- Cloud fills mix more strongly over a dark canvas (slice 55). The developer's
  tuned appearance had been read on egui's light canvas.
- The editor no longer locks the CLI out (slice 56).
- The **Graph design tree** window is removed (`higod-rugar`). `theme.toml`
  settles box / attached tab / nested family, and the reviewed Dynamic policy
  is live in `ParentTheme`, so only the sketch went. Sections below that
  describe the sketch are history; the code is recoverable from commit
  `1f3d7b8` and earlier.

Decided, not yet done (ledger items under the GUI sprint `kufod-mogar`, tagged
`tag/work/apps/todo-app/gui`):

- One normalize algorithm (`hurik-duliv`). Today they
  differ: G over a card runs `tree_positions_at_levels` with parents only, empty
  levels, no `done_above`, no `compact_active` packing, and no framing, using
  `leaf_gap`/`row_gap`/`tree_axis`, anchored at the root. G over the canvas runs
  `hierarchy_layout` or `layered_positions` per `layout.style`, follows
  dependencies and blockers per config, then separates, relaxes, and frames.
  Unify on the whole-graph pipeline restricted to the family, keeping only the
  anchoring; also let relation edges (leads-to, relates-to) inform ordering.

Unverified: `mode = "system"` applies the palette only to the appearance that
was current at load (marked `ponytail:` in `apply_colors`).

The stable names and visual contracts for family frames, anchors, tree lines,
and tag clouds live in
[Graph Design Language](gui-graph-design-language.md). Read it before changing
graph rendering; this pickup focuses on implementation state.

## Visual review, September 30

The current live family rendering was rejected in the app when a bounded board
was empty while its full-size children remained outside it. Dynamic presentation
now groups only actual nearby direct children: a cluster below its threshold is
tree-only, a nearby aligned boxed cluster gets a narrow direct rim join, and a
remote boxed cluster keeps the ordinary family tree. An explicitly Boxed family
still draws a true, potentially wide frame around all of its direct children.

The reviewed Dynamic policy is now serialized in `ParentTheme` and exposed in
Settings → Families, saved via the ordinary theme workflow.

## Pickup: the current family-design decision

The developer rejected the first simple A/B/C sketches because they changed
the displayed situation between alternatives and did not reproduce the real
empty-board failure. The design tree is intended to vary one visual rule while
holding task identities, positions, parenthood, and dependencies fixed. Keep
that invariant when extending it. Do not infer visual acceptance from unit
tests or from this document; the next step is review in a running window.

The latest design direction, as stated by the developer:

1. Keep **Dynamic** alongside explicit Box and Tree behavior. When at least
   `n` children are within a proximity range `m` (both configurable), form a
   box for that group; other children remain connected by the family tree.
   A very remote group may need a tree route to its box, controlled by a
   separate distance threshold.
2. A parent card normally connects physically to a nearby box rim. The width
   of that join should be a configurable ratio of card width, including 0–1
   and values above 1. When the parent is already inside its box, no external
   join is needed. A disconnected box should be reached by an ordinary family
   tree, not by another special-purpose connector. Minimize the number of
   distinct visual primitives.
3. Nested family frames and child-well background are independent settings.
   For now the preferred starting appearance has both enabled. Disabling a
   nested border must not change the membership of the outer family.
4. The distance/remote behavior in point 1 remains configurable, not a fixed
   one-off portal treatment.

The live and sketch rule both interpret `n` as a count of *direct* children and
`m` as a center-to-center distance. Proximity is transitive: A near B and B
near C join one cluster even when A is farther from C. A box gets a direct rim
join only when the parent overlaps its horizontal projection and the physical
gap is no larger than `parent_join_max_gap`. A parent inside its own frame gets
no extra rim join. Otherwise the existing angular family tree is the sole
structural route. The remote group never becomes a long, sideways slab merely
because the distance value is raised.

### Implemented appearance defaults

The developer later authorized live implementation. These are defaults, not
constants: each is persisted in `[parent]` in `theme.toml` and is editable in
Settings → Families.

| Theme key | Default | Live behavior |
| --- | --- | --- |
| `dynamic_min_children` | `3` | A Dynamic direct-child cluster must contain at least this many cards before receiving a frame. |
| `dynamic_cluster_distance` | `300.0` | Proximity range `m` in display units. |
| `parent_join_max_gap` | `30.0` | Maximum physical parent-card to frame-rim gap for a direct join. This is the implemented meaning of the reported `30`. |
| `parent_join_width_ratio` | `0.30` | Direct join width divided by parent-card width. |
| `child_style` | `nested_family` | Nested family treatment starts enabled. |
| `well_fill` | `dark` | Child-well background starts enabled. |
| `child_depth` | `0` | All child generations are visible by default. |

`max_board_span` is retained only so old theme files keep loading. It is no
longer shown in Appearance or used by Dynamic grouping; `n` and `m` determine
which child groups receive a frame.

### How the sketches were inspected (window removed 2026-10-14)

Steps 1-4 and 6-7 describe the removed **Graph design tree** window and are
kept as the record of how the live defaults were chosen. Step 5 still applies
to the live ledger.

1. Restart the app with `./dev gui`; the old running binary will not pick up
   source edits. Open **Graph design tree** from the toolbar.
2. **First set · structure** retains the earlier tree/box/hybrid comparison,
   parent attachment alternatives, child-space alternatives, and remote-child
   alternatives. It is a historical comparison, not the new preferred rule.
3. **Next set · dynamic families** is a static comparison fixture. Its controls
   are independent from the saved live theme; use it to compare alternatives
   without moving ledger tasks. The five direct children, grandchild,
   other-family card, and blue dependency arrow must stay fixed as controls
   move.
4. Try `n = 3` and `m = 300`, then sweep `m` through the two-group merge point.
   Observe when a child changes from tree-only to boxed. Try join width `0`,
   `1`, and above `1`; its ends must stay on the box rim. Turn nested frames
   and background off independently, then show tag context: an open well must
   be a true hole, not canvas paint over the tags.
5. Return to the live ledger and compare a parent with two nearby children,
   a parent with one distant child, a parent among its children, and a nested
   family. In the current live Adaptive fallback, an empty frame must not
   remain near a parent while its children sit outside it. A right-click
   **Family display → Box** should instead enclose the real children, even if
   that makes a large frame; **Tree** should show no family frame.
6. **Tag clouds · decision tree** keeps a work hierarchy and a competing
   `personal/fun` task fixed while comparing cloud scope, hierarchy color,
   competing-tag separation, and same-tag neighbor-link density. It is a
   chooser only: do not treat its settings as live theme behavior.
7. **Next set · tag hierarchy** is the complex follow-up: it shows the
   `work/cdint`, `work/cdint/api`, `work/docs`, and `work/ops` branches
   together, continuing `cdint` through `transport` and `codec` for five
   levels. Its defaults are hierarchy depth **All**, bounded branch-hue coloring, a
   foreground `personal/fun` cloud with cutout on and outline off, and five
   nearest same-path neighbors. It also compares all-within-radius links and
   optional local gap fill; zero is available to remove cloud rails. It has no
   parent-rail option. Its settled choices are now live Appearance/theme
   defaults: full hierarchy, deterministic sibling hue spans relative to each
   top-level group's wheel span, five nearest
   neighbors, local gap fill, and cutout of hierarchy layers a task does not
   carry; outline remains off by default.
   Ancestor cloud membership includes visible descendants, including the
   `transport/codec` and `transport/claude` leaves, so the deepest hierarchy
   has real same-path links to inspect.
   A deeper cloud must yield to a card's closest carried ancestor field, never
   to raw canvas white. Cards remain unchanged: multiple independent tags are
   represented as one repeated diagonal stripe texture in their surrounding
   circular cloud member, painted above rather than within each ancestor.
   Ancestor
   cloud members are padded more heavily than their descendants, keeping the
   hierarchy's larger parent field readable.

The current sketch cannot prove that parent-inside-box routing, collision
avoidance, or zoom scaling looks good across arbitrary real graphs. The live
theme/API names and defaults are now in place; the next review should validate
those choices against a real ledger and decide whether the direct-child,
transitive-proximity rule needs to change.

### Code and state map

- `crates/todo-gui/src/main.rs`: `family_boxes` computes live Dynamic
  clusters and frame eligibility;
  `draw_family_tree` and `family_tree_geometry` draw live family routes. The
  live renderer uses a direct rim join only for a nearby aligned frame and has
  no detached Outline Anchor.
- `crates/todo-gui/src/theme.rs`: `ParentTheme::default().child_depth = 0`
  means **All**. The live `family_view` enum remains Adaptive/Boxed/Connected;
  `dynamic_min_children`, `dynamic_cluster_distance`,
  `parent_join_max_gap`, and `parent_join_width_ratio` are persisted theme
  fields.
- The developer's `~/Library/Application Support/todo-ledger/theme.toml` was
  inspected read-only and had no explicit `child_depth` key, so reloading a
  newly built app should use the new All default. A different theme file with
  an explicit depth still overrides it.

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
  reaches the box from the canvas, Escape clears it, and non-matching cards dim
  rather than vanish so edges survive a query. Enter selects and centers the
  first visible match; subsequent Enter or Next steps forward, while Shift+Enter
  or Prev steps backward. Navigation starts only after Enter. Info's Center
  button centers the selected card without changing zoom or its saved position.
  A nonempty query temporarily reveals matching cards and their ancestor paths
  even inside hidden families; clearing it restores the prior family visibility.
- Both sidebars collapse from the toolbar.
- The centered **New task** window accepts a name, an optional existing parent
  and one new child, person with role, due date, tag, and note. **Save** closes
  after creation; **Save + New** clears the form for another task. **Edit more**
  saves, selects and centers the new card with its gold outline, opens Info,
  then briefly outlines that pane. If necessary it relaxes focused, agent-only,
  or tag-only filters so the new card is visible. Invalid
  dates, tags, and people are rejected before creation. Title, tag, and parent
  are created together; if a later detail fails, the task remains created and
  the editor reports the partial result instead of inviting a duplicate Save.
- On macOS, Apple Symbols is appended as an outline-font fallback after egui's
  bundled fonts. This covers missing symbol glyphs such as checkbox controls;
  it does not enable Apple's color-only emoji font or guarantee every emoji
  sequence in user task text. The earlier text labels remain as a reliable
  control fallback.
- Opening or closing a drawer keeps cards at their current screen positions;
  the canvas resize no longer silently reflows the default grid.
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
- Parent containment boxes wrap nearby descendants and yield to the family
  tree when the full child set would exceed the configured frame cap. The
  parent card stays at its tree-assigned head position. Every tag cloud anchors
  to the measured card center, including parent cards, so no cloud is left at
  a former layout position.
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
  is now All. A collapsed parent shows an expand badge, while active and
  selected paths remain visible. A distant child no longer produces an empty capped
  board; the family tree continues to its actual card instead. A forced Box
  draws the complete span. A short stalk attaches a detached parent card to
  a genuine nearby child outline.
- Every parent has one child-side control that displays its current local state
  and cycles `+` → `-` → `-*` → `+` when clicked.
  `-` hides inactive descendants even when child depth is All, preserving
  active-task paths; `-*` hides every descendant; `+` restores the whole
  branch, including nested hidden families. A branch hidden only by the global
  depth cap still displays `+`. Hiding a selected descendant selects its parent.
  Right-click also offers **Family display → Box / Tree**; that display
  choice is separate from visibility.
- G over empty graph canvas normalizes the visible graph. G over a card, or its
  context-menu command, normalizes only that connected parent/child tree and
  leaves unrelated families in place. There is no `gg` chord. E over a card
  expands its connected family; E over empty canvas expands all eligible
  families. The next E restores the exact preceding family visibility, and
  Undo layout also reverses either E action. C collapses inactive descendants
  under the pointer, or all eligible trees from empty canvas; a second C within
  half a second fully collapses the same scope, including active paths. Focus
  separates wrapped active rows, places active-holding parents with them, and
  omits spanning family fills.
- Shift-click toggles cards in a temporary graph selection. Shift-drag draws
  an additive selection box; an ordinary card click clears the multi-selection.
  When cards are selected, C, E, and G use only those cards as their scope:
  C/E change their descendant visibility, while G arranges just the selected
  cards and leaves every other card in place. The Info pane follows the last
  card added, or another selected card when that one is removed.
- Cloud fills are pre-blended once against the active canvas and drawn opaque, so
  a same-tag circle and its connecting tube remain one color at every overlap.
- Appearance lives in the Settings window's Cards, Tags, Families, and Motion
  tabs (formerly one sidebar section); Layout holds hierarchy depth and
  arrangement behavior. Cloud members may be
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
- The per-frame graph path now skips descendant-weight computation unless card
  size or a color channel uses weight, reuses one weight per card when enabled,
  indexes family depth once, and selects only the configured nearest tag-cloud
  links instead of fully sorting every candidate. These are source-level hot-path
  reductions, not a measured frame-time claim. For a running-window comparison,
  toggle Settings → Motion → Glide to new positions to distinguish deliberate
  easing from slow drawing, then toggle tag clouds to isolate cloud rendering.

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
46. Adaptive family display, per TE-0015. The original implementation used a
    bounded continuation board when children were distant. Visual review later
    rejected that board because it could be empty while the children remained
    outside. Adaptive now omits the frame in that case and keeps the actual
    family tree. A forced Box still shows literal containment.
47. Configurable child depth from one to six generations, or All, now defaulting
    to All. Collapse is a view layer only: the paths to active and selected work stay
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
54. `[colors]` picks dark (default), light, or system egui visuals rather than
    following the OS, since cards are drawn for a dark canvas. `palette_file`
    reads a Ghostty-format theme; its `background`, `foreground`,
    `selection-background`, and `cursor-color` restyle panels, canvas, text,
    selection, and links. Applied on the first frame and after Reload theme.
55. Cloud fills pre-blend `tag.cloud_mix` percent of the tag color over a light
    canvas and `tag.cloud_mix_dark` over a dark one, chosen by canvas luminance.
56. The editor holds a `LedgerAccess` (path and actor), not an open `Ledger`,
    and opens the ledger per operation through `with_ledger`; Turso locks the
    file for as long as it is open. It refreshes when the database or its WAL
    changes, polled once a second (`ponytail:` note; a file watcher is the
    upgrade). Covered by the CLI test `a_dropped_ledger_lets_another_process_write`.
57. The theme reloads itself when `theme.toml` or its `palette_file` changes,
    checked in the same once-a-second loop. A stamp is the resolved path,
    modification time, and size, so a home-manager generation switch, which
    only re-points a Nix store symlink, still counts. Saving from the panel
    records its own write as seen. Unsaved panel edits (live theme differs from
    `theme_loaded`) block the reload and report it instead. A deleted
    `theme.toml` is left alone. Covered by `a_file_stamp_sees_edits_and_repointed_links`.
58. Attribution (protocol v15). The inspector's **People** section lists the
    task's roles (author, contributor, assignee, requester, or any typed role)
    with Remove, adds one from a role menu and an actor name with suggestions
    from existing actors, and says `Agent-created: <name>` when an agent signed
    the creation or the author is an actor of kind agent. Cards append
    `· agent` to the meta line. **Tasks shown → Agent-created** shows, hides,
    or isolates agent tasks; the default comes from `agent_items`. The editor
    always writes as the operator, even when launched from an agent's shell.
    Covered by `agent_tasks_are_marked_and_filtered`.
59. Layout (TE-0018). Three surfaces, one job each:
    - **Filter drawer** (left, `F`): Tasks shown, Tags, Lines drawn, Overlays.
    - **Settings** (floating window, `,` or `⚙`): tabs Layout, Cards,
      Families, Tags, Motion, Colors, Inspector; the footer says whether the
      live theme differs from `theme.toml` and holds Revert, Save, and Reload
      from disk.
    - **Inspector** (right, `I` or `▣`).
    The toolbar is `+ New`, `Filter · n`, `Layout ▾` (the arrangement
    commands), zoom with its percentage, `⌖` reset view, search, `⚙`, `▣`.
    Reload theme and Refresh left it; both happen on their own. A chip row above
    the canvas lists each filter that differs from a fresh window; a chip clears
    its filter and `clear all` is Reset filters, which now returns the agent
    filter to `agent_items`. Covered by
    `each_changed_filter_is_one_chip_that_clears_itself`.
60. Inspector, read first (TE-0018). A header with the title (click or `✎` to
    edit; Enter or click-away saves, Escape restores), state and priority chips
    whose menus save on choice, complete or reopen, the spoken id with copy, and
    the parent as a link. Then five bands, each a colored stripe and label with
    a summary while collapsed and a `+ ▾` add menu: **People**, **Time**,
    **Structure**, **Context**, **Notes**. Only set fields get a row; a row
    edits in place on click and offers `✕` on hover. Colors are
    `[colors.inspector]` hex keys; `structure` defaults to the family connector
    blue. A quiet footer holds history (loaded on open), raw JSON, weight, and
    the full id. No Save buttons remain. Covered by
    `time_values_read_as_spoken_dates_and_durations` and
    `the_structure_band_shares_the_family_blue_and_bad_colors_fall_back`; slices
    59 and 60 are unverified in a window.

## Configuration

Presentation lives in `theme.toml` beside `config.toml` in the config directory.
A missing file keeps the defaults; a malformed file or an unknown key is reported
rather than ignored. Settings → Reload from disk re-reads it in place, and writes the current
defaults the first time as an editable starting point.

Sections: `[node]`, `[edge]`, `[tag]`, `[layout]`, `[focus]`, `[timeline]`,
`[parent]`, `[weight]`, `[inspector]`, `[colors]`. `[colors]` takes `mode`
(`dark`, `light`, `system`) and `palette_file` (a Ghostty theme path, `~/`
allowed), and `[colors.inspector]` takes `#rrggbb` band colors `people`,
`time`, `structure`, `context`, and `notes`; see `docs/themes/README.md`. In `[tag]`, `cloud_mix` and
`cloud_mix_dark` set cloud fill strength over light and dark canvases. In `[parent]`, `header_style` accepts
`header` or `attached_tab`, and `child_style` accepts `recessed_board` or
`nested_family`. `well_fill` accepts `transparent` or `dark`. `family_view`
accepts `adaptive`, `boxed`, or `connected`, and `connector_style` accepts
`fill_core` or `blue_core`. In `[tag]`, `cloud_member_shape` accepts `circle`
or `card` when `region_shape = "cloud"`; the default remains `circle`. In
`[node]`, `soften_outline_in_family` defaults to `true`; optionally set
`hide_outline_in_family = true` to remove only redundant same-family outlines.
In `[parent]`,
`clip_well_below_attached_tab` defaults to `true`, keeping an attached parent
card out of its dark child-space inset. `outline_weight` controls the structural
parent-outline emphasis independently of card and child-well strokes. Example
arrangements live in
`docs/themes/`, including the visual channels: a node's bar, body, and border are
each told what they stand for rather than being wired to one meaning. A `[safety]` section from an earlier
version is still accepted so existing files keep loading, but it no longer does
anything: deleting left the editor.

Values that carry data or protocol meaning are deliberately not configurable.

## Current verification state

```sh
./dev check   # cargo fmt --check, then clippy --workspace --all-targets -D warnings
./dev test    # full workspace suite
```

`./dev check` passed after the live Dynamic-family update. A full `./dev test` run
passed all 227 tests before the final rim-attachment guard. The next full run
failed only `todo-cli`'s `simultaneous_readers_wait_for_the_local_ledger`:
Turso reported that its temporary `ledger.db` was locked by another process.
The failure does not implicate the GUI rendering code, but the **current full
suite is not green** until that test is rerun successfully. The GUI unit tests
passed in a subsequent focused run of
`nix develop --command cargo test -p todo-gui --bin todo-gui` (155 passed,
0 failed) after the Dynamic-family update. Do not silently treat the CLI lock
as a passing test.

On 2026-10-25, before the latest GUI changes, `./dev check` and the full
259-test `./dev test` passed on October 26, including
`simultaneous_readers_wait_for_the_local_ledger`. After the GUI changes,
`./dev check` also passed. The earlier lock failure above remains historical
evidence. Search navigation, Info labels, palette/system appearance, symbol
fallback, the centered creation window, and Edit more's visual handoff still
need running-window review. The graph hot-path changes also need a subjective
drag, pan, and zoom comparison on the live ledger.

The editor's own coverage is the pure layers only: `geometry.rs`, `theme.rs`,
`palette.rs`, `tags.rs`, and free functions lifted out of `main.rs` for the
purpose, such as `matches_query`. Nothing drives an actual window, so any change
to panel wiring, hotkeys, or focus handling has to be confirmed by running
`./dev gui`. The following have not been exercised in a running window and are
what to check there first:

- the TE-0018 layout (slices 59 and 60): `F`, `,`, and `I` toggles; filter
  chips; the Settings tabs and footer; inspector bands, their stripes and
  collapsed summaries; in-place edits for title, dates, people, links, tags,
  and notes, including Escape restoring a date and the calendar beside it;
- the `/` hotkey and its focus handoff;
- `G` over a card versus empty canvas; verify a second `G` has no special
  chord behavior. `E` expands the hovered family or all from empty canvas;
  another `E` restores the prior visibility, and Undo reverses either action;
  `C` then a quick second `C` collapses inactive then all descendants in the
  original scope, even if the first press changes the card under the pointer;
- Shift-click two cards, then Shift-drag across others; verify the gold outlines,
  no canvas pan or card movement during the marquee, and selected-only C/E/G;
- whether a family band now reads as a border at every parent-to-child distance,
  which is the TE-0015 image 9 case;
- the dashed boundary on an open family, which appears only mid-drag;
- whether the connector casing separates families legibly, and whether
  `blue_core` reads better than the `fill_core` default against a dark fill;
- the three `family_view` states against a real ledger, especially Connected on
  a dense graph;
- whether a nearby aligned parent’s direct rim join reads as containment, and
  whether a remote group remains legible with the ordinary family tree alone;
- smooth versus card-shaped clouds over a single card, a row of matching cards,
  and overlapping unrelated tags; confirm labels stay with their clouds;
- scroll-wheel and trackpad zoom on the canvas: it should stop as soon as the
  physical gesture stops, while still anchoring the hovered card;
- Hide / Show on a parent with inactive and active descendants: inactive cards
  should disappear, the active path should remain, and Show should restore the
  branch; confirm a selected descendant hands selection to its parent;
- the compact chevron child-depth control at both a parent card and its attached
  family frame, plus same-family outline softening with and without selection;
- the TE-0015 screenshot cases: normalize a large family, expand one child,
  arrange that family from its context menu, then switch to Focus and confirm no
  inactive card sits inside the gold boundary.

Expansion and explicit hide state are deliberately in memory only. Neither is
written to `gui-layout.json`, so a restart returns to the configured child
depth. Both are covered by Undo layout; node offsets still persist.

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
8. Visually review the new Dynamic experiment before wiring `n`, `m`, and
   connector width into the live renderer. Separately decide whether Focus
   should regain a thin family breadcrumb and whether All child depths remains
   readable on a large real ledger. These need eyes on a running window.

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

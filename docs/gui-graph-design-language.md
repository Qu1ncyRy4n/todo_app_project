# Graph Design Language

This is the shared vocabulary and visual contract for the todo graph. It is
intended to make design reviews and renderer changes refer to stable parts of
the picture instead of overloaded words such as “box,” “outline,” or “line.”

It records both current behavior and settled target behavior. Requirements
called out as **planned** are not claims about the current renderer.

## Reading order

The graph must communicate in this order:

1. task cards and parent/child ownership;
2. family structure and continuation;
3. dependency relationships;
4. tag context.

Tag clouds are ambient background context. They must never read as a family
frame or obscure the parent/child relationship.

## Terms

| Term | Meaning |
| --- | --- |
| **Task card** | The rendered todo itself: title, state/meta line, status, and tag accent. |
| **Tag cloud member** | The circle or rounded card-shaped background around one tagged task card. |
| **Tag rail** | The background connection joining nearby members with the same tag. |
| **Tag label** | The tag name attached to its cloud, not a label for a parent family frame. |
| **Parent outline** | The thick structural border that encloses a parent’s child space. It surrounds children, not the parent card. |
| **Parent tab** | A parent task card when it naturally touches the parent outline. |
| **Direct rim join** | A short, configurable-width physical join from a nearby, aligned parent card to its parent outline. It is omitted when the parent is already inside the frame. |
| **Detached family route** | The ordinary angular family tree that reaches a remote child group or its box. It replaces the old special-purpose Outline Anchor. |
| **Child well** | The inner compartment bounded by the parent outline. |
| **Child infill** | Optional dark fill within the child well. It never leaks into the parent-card area or beyond the well boundary. |
| **Family trunk** | The segment from a parent card or direct rim join to a branch junction. |
| **Family junction** | An open-circle branch point in a family tree. |
| **Family branch** | An angular parent/child connector from a junction to a child. |
| **Family terminal** | The open circle at the child-side end of a family branch, placed at the top of a vertically arranged child card. |
| **Dependency edge** | A direct arrow for `depends_on`/blocking semantics. It remains distinct from the family tree. |
| **Hidden-child marker** | A small `…` at a parent card’s child-side edge. It appears only when children are hidden and reveals them on click. |

## Earlier family geometry proposal (not accepted)

```text
             [ parent task card ]
                    │
                    │  outline anchor, only when detached
                    │
  ┏━━━━━━━━━━━━━━━━━┷━━━━━━━━━━━━━━━━━━┓  parent outline
  ┃                ○                   ┃  family junction
  ┃               ╱ ╲                  ┃
  ┃              ○   ○                 ┃  child terminals
  ┃            [A]   [B]               ┃
  ┃          child well / infill       ┃
  ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
```

This diagram records the earlier intended reading, not an approved final
design. The parent outline is much thicker than ordinary card and well strokes.
Its thickness and dark infill are bounded by its actual child frame; a remote
parent does **not** pull a thick horizontal band across the gap. A nearby,
aligned parent can use a narrow direct rim join; a remote group is reached by
the ordinary family tree.

When a parent card naturally attaches to the frame, it acts as the parent tab
and no separate outline anchor is drawn. The child infill begins below the tab;
it does not paint behind or past the card.

**Implementation status:** the renderer no longer makes the legacy detached
Outline Anchor. Dynamic grouping computes frames around actual direct-child
clusters only, uses the direct rim join only for a nearby aligned frame, and
otherwise leaves the ordinary family tree in charge. This still needs a visual
review on a real ledger; compilation and unit tests do not establish that a
particular live arrangement reads well.

## Family tree contract

Draw exactly one family path for each structural parent/child relationship.
Remove a duplicate diagonal when an angular family trunk/branch already carries
the same ownership relationship. Preserve a true dependency arrow between the
same tasks when one exists: it has different meaning and remains a separate,
direct arrow.

A multi-child family shares one trunk and junction, then fans out through
angular branches. Branches meet the top edge of their child card at an open
circle. They must not be replaced by several independent diagonal arrows.
For a detached child group, one ordinary family-tree branch reaches the group
or its box; do not draw both that route and a separate anchor. A direct rim
join is only the nearby aligned alternative, never an additional relationship
path.

The family-colored core may continue through the child well. The shared blue
casing/outline is visible only outside the child infill, where it separates the
family tree from the canvas. This is the **dynamic family core** rule.

**Implementation status:** the family-colored core and geometric casing clip
are implemented. The visual review found ambiguous paths around the now-removed
capped boards. A clipping test alone does not establish that a displayed branch
communicates parenthood clearly.

## Family presentation policy

The eventual policy has one inherited graph default and an optional override
per parent family:

| Policy | Meaning |
| --- | --- |
| **Dynamic** | Cluster nearby direct children by configurable count and range; use direct-rim boxes, detached boxes reached by a tree, and tree-only children as geometry warrants. |
| **Box** | Force a true parent outline containing every visible child, even when the family is wide. |
| **Tree** | Suppress the parent outline and show only the family trunk, junctions, branches, and terminals. |
| **Inherit** | Use the graph default. This is the default for each family. |

The current renderer has a graph-wide theme default plus a right-click
per-parent override. `Inherit` removes the local entry; `Dynamic`, `Box`, and
`Tree` persist in `gui-layout.json`, are presentation-only, and affect only
that family's frame behavior. Box can become wide; use it only when literal
containment is more important than compactness. Dynamic is the live `Adaptive`
implementation. It uses the saved theme values documented below; the
per-family override changes only the presentation policy, not those shared
appearance values.

## Tag cloud contract

Each task contributes one cloud member from its deepest effective tag. A cloud
uses fully pre-blended color so overlapping members and rails of the same tag
remain one continuous color rather than darkening. Different tag clouds stay
visibly separate.

Cloud members may be circular or card-shaped. Card-shaped members follow the
measured card rectangle, including wrapped-title height, with configurable
padding. They annotate cards; they do not form a child well or parent outline.

## Graph design comparisons

The **Graph design tree** window was removed on 2026-10-14 once its choices were
settled in `theme.toml`; this section records how those choices were compared.
It used one multi-generation dummy graph with
two fixed arrangements: parent above, or parent among its children. A near
child, a distant child, a grandchild, a cross-family neighbor, and dependency
arrows stay in the same positions as choices change. Step through four
decisions: tree/box/hybrid structure; tab/anchor/continuous attachment;
open/recessed/nested child space; and outside/span/portal treatment of the
distant child. Tags can be toggled without altering structure. This is an
interactive design sketch, not the production renderer; it reads or writes no
ledger data or saved appearance settings.

The live adaptive renderer no longer paints a capped frame whose visible
children are outside it: that family uses its actual tree instead. A forced
Box retains a genuine frame around every visible child, even if wide. These
are stopgap truthful representations while the decision tree is reviewed;
they do not settle the preferred visual treatment.

### Tag-cloud decision tree

The **Tag clouds · decision tree** page uses one fixed tag fixture. It keeps
three `tag/work/cdint` tasks, two other `tag/work/*` tasks, and one
`tag/personal/fun` task in the same positions. The personal task deliberately
sits inside the broad work area, so every alternative must answer how a
competing hierarchy stays readable without moving the task.

The page varies four independent decisions:

1. **Scope:** deepest path only; broad parent cloud plus child cloud; or a
   broad parent rail plus child cloud.
2. **Hierarchy color:** same hue; depth shade; or a more distinct child-branch
   hue.
3. **Competing tags:** ordinary blending; a foreground island; or a cutout in
   the parent field beneath the competing cloud.
4. **Neighbor links:** current-like minimum tree; all nearby pairs; or a dense
   local neighborhood.

It is a visual chooser only. None of these tag decisions changes the live
renderer, saved theme, filtering rules, or task positions until explicitly
approved after review.

### Next tag decision set: hierarchical branches

The **Next set · tag hierarchy** page is the more demanding fixed fixture. It
contains `work/cdint`, `work/cdint/api`, `work/docs`, and `work/ops` at once,
with `personal/fun` placed inside the ancestor `work` field. It has no
parent-rail option.

Each displayed ancestor cloud includes every visible descendant in that path.
For example, `work/cdint/api/transport` surrounds its own card together with
the `codec` and `claude` leaf cards. This makes deep hierarchy connections
visible instead of rendering every deep path as an unrelated singleton.

Its defaults are **All** hierarchy depth, the full ancestor-to-descendant
cloud stack, bounded branch-hue coloring, a foreground `personal/fun` cloud with
**Cut out hierarchy fields behind it** on and its outline off, and **5**
nearest same-path neighbors. Branch hue drifts only within the work
blue–teal–green neighborhood: deeper `cdint` trends green then teal, while
sibling branches may lean blue or teal without becoming unrelated categorical
colors. Cutout and outline are independent controls. The
depth control accepts a positive number of painted levels or **All**; it
changes cloud context only, never task tags.

Same-path rails can use either **N nearest neighbors** (with `0` leaving
members unconnected) or **all within radius**. A separate local-gap fill can
make a multi-member cloud continuous between cards; it applies only inside one
tag path, so differently tagged branches remain visually distinct.

When a deeper cloud crosses a card that does not carry that deeper path, its
cutout restores the card's closest carried ancestor field instead of canvas
background. For example, a `work/cdint` card remains in the `work/cdint` field
where a nearby `work/cdint/todoapp` cloud yields. Cards remain unchanged; when
a task has independent leaf tags, the surrounding circular cloud member is
filled with one repeated opaque diagonal stripe texture of their colors rather
than blending colors or adding a card cue. The texture is painted once above
the hierarchy, never once per inherited ancestor field.
Ancestor cloud members receive progressively more padding than descendants, so
the parent field stays visibly larger behind the child field.

This fixture settled the live tag-cloud defaults: full inherited hierarchy,
deterministic sibling hue spans relative to the top-level group's available
wheel span, five nearest same-path neighbors,
local gap fill, and cutout of hierarchy layers a nearby task does not carry.
Settings → Tags exposes hierarchy depth, sibling hue spread, depth
hue drift, nearest-vs-radius links, local gap fill, and cutout/outline toggles.
The wheel is rebuilt from visible tasks on every frame. Its root and sibling
spans are weighted by visible terminal tag assignments, so filtering to a view
such as `work` reallocates the available color breadth among that view's active
leaf branches rather than reserving equal space for off-screen or sparse work.

### Next decision set: dynamic family grouping

The **Next set · dynamic families** page is another fixed visual experiment.
It has five direct children in two potential proximity groups, one grandchild,
and a separate task with a dependency arrow. The cards do not move as values
change. Its provisional rule is:

1. Join direct children transitively when their card centers are within `m`
   display units. A resulting group receives a box when it has at least `n`
   direct children; smaller groups remain tree-only.
2. A box near the parent whose rim overlaps the parent card's horizontal
   projection connects directly to the card. The join width is a configurable
   ratio of card width, from a thin line through greater than full width.
3. A distant or sideways-displaced box is reached by the ordinary angular
   family tree. It gets no separate special anchor. The near/far distance is
   independently adjustable.
4. A nested family frame and the child-space background are independent
   switches, both on by default in this sketch. Turning off the nested frame
   does not remove the grandchild from its outer family's containment.

This expresses the three live Dynamic outcomes—**connected box**, **detached
box**, and **tree only**—with fewer connector primitives. The live renderer
uses the same core rule the sketch did: direct-child count and transitive
center-to-center proximity.

The live default configuration, all exposed under **Settings → Families**, is:

| Theme key | Default | Meaning |
| --- | --- | --- |
| `dynamic_min_children` | `3` | Direct children required for a Dynamic cluster to receive a box. |
| `dynamic_cluster_distance` | `300.0` | Display-unit, center-to-center proximity range `m`. Proximity is transitive. |
| `parent_join_max_gap` | `30.0` | Maximum clear gap between a parent card and a nearby frame for a direct rim join. |
| `parent_join_width_ratio` | `0.30` | Direct-rim join width as a fraction of parent-card width. |
| `child_style` | `nested_family` | Nested family treatment is on. |
| `well_fill` | `dark` | The child well has a background by default. |

The first four values, nested/recessed child treatment, and dark/transparent
well fill are configurable in `theme.toml` and Settings → Families.
Existing `max_board_span` remains a compatibility key for old themes and is no
longer an active Dynamic control.

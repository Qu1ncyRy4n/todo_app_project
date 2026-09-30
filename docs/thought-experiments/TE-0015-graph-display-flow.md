# TE-0015: Family Display Flow and Focus Layout

Date: 2026-09-29
Status: implemented through a second pass; visual review still pending

## Decision Under Test

How can the graph show that a task owns a child subgraph without stretching a
large opaque frame across distant work, and still make Focus a readable view of
what is active now? This includes how much child depth is visible and what a
per-task arrangement command would move.

This experiment proposes display behavior only. It changes no task identity,
parent relationship, dependency, saved item state, or protocol event.

## Observed Starting Point

- Screenshot 1 has a parent card partway through its descendants rather than
  unmistakably attached to the family boundary. A nested frame spans remote
  cards, creating a large dark slab whose ownership is difficult to trace.
- Screenshot 2 shows non-focus cards and multiple family fills inside the Focus
  Window. The visual result is a dense stack, not an isolated active region.
- `family_boxes` unions *all* descendant card rectangles and nested family
  bodies. `family_box` then spans from the parent card to that union. This is
  why one far child can enlarge the whole board.
- `focus_layout` places only explicitly Active tasks in its active band, but
  focus-frame drawing also includes tasks that *hold* active work. A parent can
  therefore enlarge the frame while placed in the future band.
- Tag separation and generic overlap relaxation run after Focus band placement.
  They do not preserve the active-versus-exiled boundary.
- The current hierarchy packer moves whole families using approximate card
  clearance, not measured final family-body bounds. It can compact centers yet
  leave visual boards intersecting.

## Design Invariants

1. A visible parent has one unmistakable anchor to its child space. Parenthood
   must not be inferred from color, proximity, or an ordinary dependency arrow.
2. Focus items occupy one bounded central region. Every non-focus *card* lies
   outside that region with measured clearance, even after any secondary pass.
3. A collapsed descendant remains discoverable through a count and an expand
   action. Collapsing changes only the view, never the ledger.
4. A local arrangement leaves unrelated families where they were, is undoable,
   and does not silently rewrite task relationships.
5. Layout uses measured card/family bounds, including wrapped titles and zoom,
   before claiming that regions do not overlap.

## Routes

### A: Unlimited Physical Containment

Keep every descendant inside one continuous parent frame. Put the parent in a
header at its nearest edge and route a short stalk from the card to that edge.
Pack the *rendered* rectangles, not just node centers.

This preserves literal containment and shows every task at once. A single
remote child still forces a huge frame, and deep nested families occupy area
proportional to their full visible subgraphs. It does not solve screenshot 1.

### B: Bounded Family Portal

Keep the parent card at its graph anchor and attach it to a bounded child board
with a distinct short spine ending in an open circle. Inside the board show the
first visible child level, with child-family previews or counts for deeper work.
Expanding a child opens its local board; expanding the whole family can use a
temporary dedicated view. Remote children never stretch the compact board.

This makes the parent easy to identify and puts a hard limit on the dark area.
It costs one interaction to inspect depth and needs a clear indication when
content is hidden. A board no larger than the parent *card* cannot physically
contain multiple full-size child cards; at that size it must be a portal or
summary, not literal full containment.

### C: Adaptive Hybrid (Recommended)

Show a continuous contained board only while measured descendants fit a modest
maximum footprint relative to the parent card. Beyond it, use route B's portal
and an explicit continuation marker. Keep the parent's card attached to the
board in either state. Never draw a giant bridge fill to a remote child.

This retains the liked nested-family design for small families and gives large
ones a compact, predictable shape. The transition point can surprise users if
it changes on every frame; it should change only on layout/expand actions and
use a stable measured threshold. The cap should be a theme preset or one
high-level size control, not several more unexplained sliders.

The external precedent is recursive compound layout: Graphviz's `osage` lays
out each cluster's internals first, then packs clusters as rectangles. Its
documentation also notes that edges are ignored during packing, so this is a
geometry precedent, not proof that cross-family dependencies will read well.
ELK supports compound graphs and cross-hierarchy edges in layered layout; that
is a candidate to evaluate against a fixed fixture before adopting a library.
Sources: https://graphviz.org/docs/layouts/osage/ and
https://eclipse.dev/elk/reference/algorithms/org-eclipse-elk-layered.html.

## Child Depth Alternatives

- **All levels:** current behavior. Complete at a glance, but deep families
  dominate the canvas and their frames compound.
- **Fixed level count:** a global default such as one level, plus per-parent
  expand/collapse. Predictable footprint; hidden active work needs an override.
- **Adaptive depth (recommended):** begin with one level, automatically reveal
  the path to active/selected work, and show counts for other collapsed branches.
  The visible path is stable until the selection or focus state changes.

The depth limit is a *rendering* limit, not a query filter. Search results and
dependency endpoints inside collapsed branches need a breadcrumb/expand
affordance rather than a disappearing edge. A per-parent override should be
view metadata, local to the GUI and reversible, not an item property.

## Per-Node Normalize Alternatives

- **Arrange this card:** changes only one position. Useful for an isolated leaf
  but ambiguous for a parent whose card is attached to a board.
- **Arrange children:** pins the parent card and rearranges its immediate child
  boards. Grandchildren move with their child board. This is the clearest
  default for a parent.
- **Arrange family:** lays out the entire descendant subgraph with the parent
  pinned, then moves that family only enough to avoid nearby measured boards.
  Potentially disruptive, so it needs an undo preview or clear scope label.

Recommended context menu: `Arrange children` and `Arrange family`, with a
single `Undo layout` path. A leaf offers `Place near parent` instead. The
menu must distinguish *arrangement* from changing parenthood or dependencies.

## Scenario Analysis

### Small Family With Two Nearby Children

Route C keeps the attached card and continuous nested board. One-level depth
already shows both children. No portal or continuation is necessary.

### One Child Far Away

Route A paints the distracting dark span in screenshot 1. Under C, the family
stays bounded; the remote child is represented by a continuation chip until
expanded or brought near. The chip states the child count and is connected to
the parent's board by the family-style spine, never a dependency arrow.

### Parent Between Its Children

A header-only box makes the parent card appear to be another child. Under C, a
short, visually unique spine connects the parent card to the board boundary.
The card receives a parent marker and the board repeats its title only if the
card is offscreen; color alone carries no ownership claim.

### Focus With Active Child And Inactive Siblings

The active child and its active-holding ancestor belong to the focus cluster.
Inactive siblings are outside the focus rectangle. The full family fill is
suppressed or split into a local focus-path frame; it must not bridge focus and
exile regions. A breadcrumb preserves the relationship to exiled siblings.

### Many Active Cards Or Deep Active Path

Measure the complete active cluster first, then place completed work above its
top and future work below its bottom by at least the configured clearance.
Fixed band offsets are insufficient when the active cluster grows. Optional
tag or overlap passes must respect the focus exclusion region or be skipped.
Zoom should frame the active cluster after this placement, without squeezing
its node spacing below readable limits.

### Failure, Recovery, And Compatibility

- If measurement or packing cannot find a collision-free placement within an
  iteration limit, keep the deterministic pre-pass layout and report that the
  local arrangement was incomplete. Do not save a partly moved family.
- If a family contains a cycle or a cross-family dependency cycle, retain all
  cards and use a visible cross-link. Do not claim a one-way flow that cannot
  exist.
- Existing `theme.toml` keys and saved node offsets should remain readable.
  New depth/size controls need defaults; old themes must not fail parsing.
- `Undo layout` restores all positions affected by one command. A local command
  must not overwrite unrelated manual placements.
- No new data leaves the machine; these are view-only operations. The only
  privacy concern is not logging task titles in diagnostic layout traces.
- Rollback is switching to the previous arrangement with `Undo layout` or
  disabling the new bounded-family style, not migrating ledger data.

## Proposed Implementation Order And Observable Checks

1. **Focus correctness:** classify derived active-holding parents consistently;
   measure active bounds; place all exiled cards outside them; prevent secondary
   layout passes and spanning family fills from re-entering the focus region.
   Test a wide/deep active fixture and assert non-focus card bounds do not
   intersect the final focus frame.
2. **Parent anchoring:** add the distinct parent-to-board spine and a parent
   marker. Test parent above, below, and between children; verify the spine
   touches both the card and its own board only.
3. **Bounded family display:** introduce a stable measured cap and continuation
   portal. Test a far child, wrapped titles, nested families, and zoom. Assert
   the board stays within its cap and hidden descendants remain discoverable.
4. **Depth and local arrangement:** implement global default depth plus
   per-parent expansion, then context-menu arrangement with one-step undo.
   Test scope isolation, active-path reveal, search, and dependency breadcrumbs.
5. **GUI review:** use a fixed fixture containing the two screenshot shapes and
   a larger real ledger. Compare frame overlap count, family-box area, visible
   active-card count, and average edge crossing count before choosing defaults.

No API or CLI behavior changes are proposed. The GUI exploration path after a
slice lands is: Normalize a large family, expand one child, right-click its
parent to arrange children, then switch to Focus and check that inactive cards
sit outside the gold boundary.

## Trial Decision (2026-09-29)

The developer approved trying route C. Child depth is settable, including All;
when a low depth hides descendants, each affected parent offers an expand
control that reveals its full subtree. G over the canvas normalizes all visible
work; G over a card normalizes that card's connected parent/child tree. The
same family action is available on the card's context menu. The initial trial
keeps the compact board capped by a parent-card size multiplier and lets its
continuation chip open the full board.

Focus exclusion and parent attachment are part of the trial, not settled visual
defaults. A running-window review of the screenshot cases is still required.

## Second Pass (2026-09-29)

The developer's annotated screenshots identified the frame problem precisely: it
was geometry, not styling. The body was the union of the well with the parent
card, so the band between them was as thick as the parent-to-child distance, and
the board cap then shrank that slab into a frame whose well sat in one corner.
The body now grows from the well alone and the card is folded in only within one
card height, which is the distance at which folding it in can still keep the band
inside the card.

Three further changes followed from the same screenshots. An open family drops
its fill entirely, because its cards already carry the structure, and shows a
dashed boundary only while it is being dragged. Connectors are drawn as a casing
plus a core so they carry both the shared family blue and the family's own fill.
A `family_view` force applies one reading, Adaptive, Boxed, or Connected, to
every family at once rather than letting each family's geometry decide.

## Still Open After The Trial

- Whether the parent-to-board stalk and continuation shape read clearly at
  different zoom levels.
- Whether Focus should regain a thin family breadcrumb; the first trial omits
  the spanning family fill entirely while keeping family links.
- Whether one generation is the right initial depth on a large real ledger.

The repo's imported cdint-grid TE procedure names selectors and DI records
unavailable in this consumer repo; no DI is claimed here. Recorded by
agent-assistant.

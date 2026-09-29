# Native Graph Editor v0

## Purpose

Provide a direct visual view of one local Todo Ledger and make a selected task
safe to inspect and edit. This is a native Rust application using `egui` and
`eframe`; it reads and writes through `todo-store`'s public `Ledger` API.

The existing `tools/todo-graph` export viewer remains a read-only portable
viewer. It is not the editor's implementation base.

## Observable behavior

- The application opens the configured local ledger and shows tasks as nodes.
- Selecting a node opens its details in the inspector.
- The inspector supports explicit edits for title, priority, valid lifecycle
  transitions, due/defer dates, tags, one structural parent, planned
  dependencies, active blockers, and
  append-only notes.
- A field never changes the ledger until its visible validation succeeds and
  the operator selects that field's **Save** action. An invalid title identifies
  the issue and leaves the durable ledger unchanged.
- Completing or reopening the selected task uses the existing lifecycle API and
  updates the graph immediately.
- A visible error state reports an unavailable or locked ledger; it does not
  silently create an alternate database.

## Relationship overlays

The initial model has two durable relationship types and they must not be
visually conflated:

| Overlay | Relationship | Direction | Mark |
| --- | --- | --- | --- |
| Hierarchy | parent to child | parent → child | green arrow |
| Depends on | prerequisite to dependent task | prerequisite → dependent | blue arrow |
| Blocked by | active impediment to blocked task | blocker → blocked task | red arrow |
| Combined | selected overlays | as above | colored marks |

Project scope and tags are filters, not relationship types. The graph can pan
with middle-drag or arrow keys, zoom with the toolbar, and move cards with
left-drag. Card positions persist locally in `gui-layout.json`; they are a
personal presentation preference and are not synchronized ledger data.

## Future focus sequences and time views

A planned dependency means the target cannot become ready until its prerequisite
reaches a terminal state. An active blocker is a separately recorded current
impediment and must be explicitly removed. Both have derived inverse views
(`dependents` and `blocks`); only one directed edge is stored. Neither is an
ordered daily plan. Focus sequences reference existing tasks in an explicit
order, allowing an interleaving such as `a1, a2, b1, b2, b3, a3`; they never
change readiness. Create one from the inspector or `todo focus create`, then
add a selected task or use `todo focus add`. A timeline view should use distinct
scheduled-date/time fields rather than overload due dates, defer dates, or
namespace paths.

## Future cross-project contributions and child completion

Structural parenthood remains a single-parent, project-local decomposition
relationship. When a task contributes to work in another project, use a future
typed edge such as `contributes_to` or `implements` rather than adding a second
structural parent. This keeps ownership, tree placement, and completion rollup
unambiguous.

When every child of a parent reaches a terminal state, the GUI should display
**All children complete** as a completion signal. It must not automatically
mark the parent done: the parent can still require review, evidence, or an
acceptance decision.

## Focus window and future work sessions

A Focus Window currently frames active tasks in the graph. A richer bounded
view should place done work above and future or deferred work below, then show
sequence neighbors, parents, dependencies, notes, and linked sources without
changing the underlying graph semantics.

Date entry has a **Today** shortcut; a native calendar picker remains future
work. Links appear in the inspector with labels and an explicit open action.
The inspector can start and stop append-only work sessions; the CLI offers
`todo session start|stop|ls` for the same durable records.

Tag paths use the canonical singular `tag/` kind, such as `tag/work/cclab`.
The filter and inspector pickers limit visible suggestions to five matching
paths. The tag constellation visualizes shared prefixes as nested colored
regions; it is a view overlay, not a second tag store.

## Explicitly deferred

- Dragging to create or remove relationships.
- Multiple structural parents, new task types, cross-project hierarchy, and
  generic typed links.
- Web hosting and a second GUI-owned persistence layer.

## Atomic multi-field editing

The raw editor uses a typed `ItemPatch` boundary in `todo-store` for scalar
title, priority, lifecycle, due-date, and defer-date changes. It parses and
validates the complete draft before committing one event/view transaction.
Relationships and append-only notes remain explicit operations. See
`gui-graph-pickup.md` for the current validation follow-up.

## Verification

The first implementation needs controller tests for title validation and the
selected-item transition mapping. Existing store tests remain the source of
truth for lifecycle, hierarchy, blocker, namespace, and event validation.

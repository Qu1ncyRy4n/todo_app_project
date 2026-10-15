# TE-0017: Recurrence and Habits

Date: 2026-10-14
Status: open; long-term

## Decision Under Test

How can habits and recurring tasks be represented so they stay cyclical and
consistent without accumulating incomplete items?

## Constraint

A recurring task that spawns a new item per cycle leaves a trail of open items
whenever a cycle is missed. Nobody wants twenty open "stretch" items. The
developer would rather see a summary than the raw nodes.

## Leading shape

- **One node per habit**, carrying a recurrence rule (Obsidian Tasks syntax,
  such as `every day` or `every week when done`).
- **An occurrence is an event, not an item.** Doing it appends an occurrence
  event, much like a session. A missed cycle is computed, never stored, so
  nothing accumulates.
- **The default view is stats**: streak, completion rate over a window, last
  done, next due. Expanding it shows the derived instance view.
- **Materialize on demand**: an occurrence that needs notes, subtasks, or
  attribution can be promoted to a real child item, linked `spawned-from` the
  habit (TE-0016 catalog).
- `todo ready` and `next` show a habit only when its current cycle is due and
  undone.

## Questions

1. Habit versus recurring task: is "pay rent" (must happen, overdue matters)
   the same shape as "stretch" (rate matters, misses are fine)? One rule with
   a `miss = overdue | skip` field may cover both.
2. Is a recurrence rule stored data, and so a protocol version, or a parsed
   field? It is stored and replayed, so it is not configurable.
3. How is a habit drawn on the graph: one node with a sparkline, or a ring?

Recorded by agent-assistant.

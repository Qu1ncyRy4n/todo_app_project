# TE-0013: Derived Activity of Parent Tasks

Date: 2026-09-28
Status: partially concluded; derived highlight implemented, state propagation deferred

## Decision Under Test

When a subtask is active, in what sense is the work above it active?

A parent holding active children is plainly being worked on, yet its stored state
may say inbox. The decision is whether activity should propagate into stored
state, be derived when read, or neither.

## Assumptions

- Structural parenthood is containment, not dependency, and does not affect
  readiness.
- State transitions are events; propagation would write one event per ancestor.
- The Focus Window shows work considered active now.
- A task may have many descendants at different states.

## Routes

### A: Derived Only

A parent with an active descendant is drawn as active and joins the Focus Window.
Stored state is untouched. Finishing the last active descendant ends the
appearance with no further action.

### B: Propagated State

Starting a child writes state-changed events for every ancestor. The ledger then
states what the graph shows, at the cost of cascade events and a manual reversal
when the child finishes.

### C: Neither

A parent stays exactly as stored and sits outside the Focus Window while its own
children are worked on.

## Scenario Analysis

### Starting One Subtask Of A Large Project

Under A the project reads as underway while its own state records that the
project itself was never picked up as a unit of work. Under B the project becomes
active and stays active after the child finishes, so a stale active project
accumulates unless someone closes it. Under C the Focus Window shows a subtask
with no indication of what it belongs to.

### Finishing The Last Subtask

A ends the appearance immediately and needs no cleanup. B requires deciding
whether completion propagates upward too, which is a second, harder question:
finishing one child rarely finishes its parent.

### Reparenting

Under A the appearance follows the current tree, because it is recomputed. Under B
the old parent keeps an activity it no longer has any claim to.

### Reporting And History

B produces a log that can be read back to ask when a project was underway. A
produces no such record: the history says only that leaves were worked on, and
anything wanting project-level intervals must derive them from descendants.

## Narrowed Conclusions

- Derived activity is correct for display, needs no cleanup, follows reparenting,
  and writes nothing that must later be corrected.
- The reporting argument for B is real but is an argument for *recording
  intervals*, not for overwriting a state field. Sessions already record intervals
  against a task and would extend to a subtree without touching state.
- C is rejected: it hides the connection between focused work and what it serves,
  which is the whole reason the Focus Window exists.

## Decision

Route A is implemented: a parent with an active descendant is drawn as active,
joins the Focus Window, and is labeled as carrying active work rather than being
active itself. Stored state is never written.

Route B remains deferred and is not foreclosed. If project-level intervals are
wanted, the next step is to test extending work sessions over a subtree rather
than to propagate state, because the value asked for is history, not status.

Recorded by agent-assistant.

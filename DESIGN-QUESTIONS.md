# Open design questions — primitives

Seed for a focused design chat in this repo. Current primitives: **Nodes /
Tags / Links**. Question: are they enough, durable, and extensible — and where do
they overlap with `teacher-bot_project`?

## Take (starting point, not settled)

- **Content, Tags, Links are close, but Links should be typed.** A plain tag is a
  bucket; a **typed link/edge** (a relation: `blocks`, `part-of`, `promised-to`,
  `derived-from`, `depends-on`) turns the graph into a *semantic* graph. This is
  the single most valuable addition and it is what makes delegation and Promise
  Theory expressible at all. Recommend: keep Nodes + Tags, promote Links to
  **typed edges** with their own properties.
- **Typed tags** (from the other chat) — worth it for canonical kinds
  (role/goal/project/status). But typed *links* buy more than typed *tags*; a
  role/goal is arguably a Node others link to, not a tag. Decide whether
  role/goal/project are nodes-referenced-by-typed-edges (cleaner) or aliased
  tags (current SPEC).
- **Time — not a fourth primitive.** Model time as typed **properties** on nodes
  and edges (`created`, `due`, `scheduled`, `done_at`) plus an **append-only
  event log** for history. First-class timestamps are needed for the time-graph
  viz and for promise deadlines, but they are fields + events, not a new node
  category.

## Delegation + Promise Theory (foundational)

Reuse the promise-evidence model from `pg_learning`
(`sessions/.../blackboard.md`, the `Promise` / `Observation` / `Assessment` Go
structs). Foundational design consequences to lock early:

- **Agents are nodes.** A delegation is a **typed edge = a promise** from one
  agent to another, carrying a body (outcome), conditions, and a deadline.
- **No global done flag.** Whether a promise was kept is a **local assessment**
  from evidence, per observer — not one shared boolean on the task. Baking a
  single `status` field into a task now will fight this later. Design the schema
  so completion is observed/asserted, not a mutable global truth.
- **Evidence is content-addressed.** An observation references evidence by hash
  (shared store with teacher-bot).

Issues to think through before building: conditional promises (who controls the
condition vs the outcome), promises to oneself vs to others, a "task market"
(advertise needed work without assigning it), and how trust/consequence of a
broken promise stays local.

## Overlap with teacher-bot

Shared substrate is in `SPEC.md`. The concrete link: **roles / goals / values /
projects live here as nodes**, referenced by the teacher bot. Keep the primitives
general enough that a "source", a "note", and a "task" are all just nodes.

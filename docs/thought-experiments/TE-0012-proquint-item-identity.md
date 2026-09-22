# TE-0012: Proquint Item Identity and Organization

Date: 2026-09-21
Status: deferred

## Decision Under Test

How should the global todo ledger assign memorable proquint references while
preserving stable item identity and useful organization?

The organization convention treats proquints as minted, durable coordination
handles. This ledger currently uses UUIDs internally and stable namespace IDs
with mutable canonical paths. The decision is whether an item handle should be
flat, kind-prefixed, derived from structure, or carry semantic organization.

## Assumptions

- UUIDs remain the internal primary key and exact event subject identity.
- A proquint is an additional durable reference, minted transactionally in the
  global ledger and unique across its synchronization set.
- Projects, tags, goals, and structural parents can change after capture.
- Structural parenthood is distinct from blocking dependencies, and an item may
  be reparented or have no parent.
- TE, DR, and DI documents remain external artifacts cited or linked from items;
  they are not new todo item primitives in this decision.
- Existing UUID-only rows can be backfilled by an automatic, callable migration.

## Routes

### A: One Flat Immutable Handle Per Item

Each item receives one globally unique two-proquint handle, such as
`babad-lusab`. The CLI accepts it wherever it currently accepts a UUID fragment.
Project, parent, and tag context appear in listings but not in the handle.

### B: Fixed Item-Prefixed Immutable Handle

Each item receives one globally unique handle rendered with a fixed kind prefix,
such as `item/babad-lusab`. The prefix establishes that it is an item reference,
not a project path or external record identifier. Organizational context remains
separate metadata.

### C: Structural Compound Handle

Each root item receives one proquint. A child receives a second proquint rendered
with its current root or parent, such as `babad-lusab/lusab-zuzuz`. The composed
reference visibly communicates containment.

### D: Semantic Immutable Handle

At creation, the application mints or selects words from the item's title,
project, tags, or category and freezes them as its handle. Later organization
changes do not alter the handle.

### E: Semantic Mutable Path

The visible item reference is recomputed when title, project, tags, or parents
change. Aliases may preserve old references.

## Scenario Analysis

### Initial Capture and Recall

Route A is concise and works for inbox items that have no organization yet. It
requires a query to learn context. Route B adds a stable visual type marker with
slightly more typing. Route C supplies immediate hierarchy context only after a
parent exists; an inbox item still needs a standalone form. Routes D and E make
the reference self-describing but require a vocabulary, normalization, and
collision policy in addition to proquint allocation.

### Reparenting and Project Changes

Routes A and B are unchanged when an item moves. Route C must either change the
visible identity, preserve an alias that no longer describes current structure,
or distinguish a stable child handle from a mutable composed display path. Route
D preserves an increasingly misleading name after reorganization. Route E keeps
the name current but makes every reference, citation, and command history
dependent on alias resolution.

### Multiple Relationships

An item can have a structural parent plus independent blockers, tags, citations,
and future typed links. Route C privileges one relationship as identity even
though it is not necessarily the work's most important context. If future work
permits multiple structural parents or a different primary view, a compound path
has no single unambiguous form. Routes A and B leave every relationship equally
available to views and queries. Routes D and E must choose which facts become
words and which remain invisible.

### Concurrent Capture and Synchronization

All routes need transactionally reserved random proquints. A flat handle can be
made unique with a database constraint and event-log convergence checks. Route
C additionally needs parent availability and a policy for two devices assigning
different parents before synchronization. Semantic routes require deterministic
normalization and collision resolution across devices; otherwise identical
titles or categories can create conflicts unrelated to the underlying work.

### Migration and Mixed Versions

Routes A and B permit an additive `handle` field and automatic UUID-row
backfill. Old UUID references remain valid. Route C needs a migration rule for
which historical ancestor supplies a root prefix and what happens when that
ancestor changes. Routes D and E require a durable vocabulary and alias ledger
before old references can be interpreted safely.

### Long-Horizon Organization and Memory

Routes A and B make handles memorable by being pronounceable, but organization
lives in views rather than in the identifier. Route C makes a local hierarchy
memorable, at the cost of stale or long references as trees evolve. Route D
preserves the initial story of an item, which can be useful historical context
but is not a reliable current locator. Route E presents current organization
well but makes durable citations and spoken references more operationally
expensive.

## Narrowed Conclusions

- A proquint should be immutable once assigned. Mutable semantic identifiers
  conflate a current organizational view with durable identity.
- A root/child compound reference can be a useful rendered path, but treating it
  as the sole durable item ID creates reparenting, ambiguity, and migration
  obligations.
- Stable handles and mutable organizational paths can coexist: the handle names
  the item, while project/tag/parent views make it memorable in context.
- The remaining decision is whether the durable form should be bare
  `babad-lusab` (Route A) or fixed-prefixed `item/babad-lusab` (Route B), and
  whether a hierarchical display alias is introduced now or only after
  dogfooding demonstrates demand.

## Implications For Pending Work

- Do not use whole-tree `mogent mint` as the item allocator. It scans generated
  build output in this repository and is not an atomic ledger reservation.
- A selected route needs a new event protocol version, a unique materialized
  handle field, replay semantics, automatic backfill, CLI reference resolution,
  and two-device collision tests.
- A future managed Markdown projection may render organizational paths beside
  the immutable handle without making paths authoritative. This refines the
  open identity question in DR-0003 rather than selecting a Markdown format.

## Refinements

### 2026-09-21 - Durable UUIDs, Human Proquint Call-Signs

The developer clarified that the proquint's purpose is quick recognition,
speech, memory, and CLI invocation, not replacement of the UUID or event CID
as backend identity. A two-proquint call-sign may therefore be stored with an
item without claiming a 32-bit space is globally collision-free. A collision
after synchronization must preserve both UUID-backed items and produce an
ambiguous-reference error until the caller supplies a UUID fragment or other
disambiguating context. This avoids lengthening or mutating a memorable
call-sign solely to simulate backend uniqueness.

### 2026-09-21 - Deferred in Favor of Interaction Design

Implementation is deferred. The desired operator workflow is incremental item
selection: a user or agent begins a command such as `todo edit prepare_...`,
then completion presents a small, relevant candidate set and permits cycling to
the intended item. UUID fragments, future proquint call-signs, title words,
project scope, and tags can all be candidate selectors without any one of them
becoming the sole durable backend identity. A later interaction-design slice
should establish completion, relevance ranking, ambiguity presentation, and
non-interactive agent output before selecting a call-sign storage or migration
format.

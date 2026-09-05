# TE-0009: Editable Markdown Manifest

Date: 2026-09-04
Status: deferred

## Pressure

Large, long-lived projects benefit from a document that can mix prose, root
work nodes, and progressively refined subnodes. Markdown should be broadly
readable and editable without becoming a second silent authority beside the
event ledger.

## Routes

### A: Markdown and Git become authoritative

This matches `todo-report` and makes branch state, review, and manual editing
natural. It would demote exact ledger events to derived history and conflict
with the current local-first event architecture. Migration and concurrent
writers would require a new authority decision.

### B: Generated Markdown remains read-only

Simple and safe, but misses the main value of Markdown: editing project
structure in context.

### C: Managed editable projection with preview and approval

Export a projection tied to an exact ledger frontier. Parse later edits into a
semantic plan, show additions and mutations, detect stale or conflicting
changes, obtain approval, append events in one transaction, then refresh the
projection. This preserves one authority while supporting document editing.

### D: `TODO/TODO.md` index plus detail files

This offers readable stable handles, Git review, focused large-project files,
and compatibility with `todo-report`. It also introduces file placement,
index/detail consistency, and branch semantics that do not currently exist in
the global ledger. It may be a concrete presentation of route C rather than a
separate authority model.

## Identity alternatives

- HTML comments carrying item IDs are visually quiet but may interact poorly
  with Markdown parsers and hand editing.
- Readable stable slugs are pleasant and linkable but require collision,
  rename, and alias rules.
- Obsidian block IDs are readable in that ecosystem but are not universal.
- Markdown links can name item URIs explicitly but add visual weight.
- A sidecar mapping preserves clean Markdown but makes moves and copies harder
  to interpret safely.

No representation is selected.

## Required behavior before implementation

- Preserve unrelated prose and formatting rather than regenerating the whole
  document destructively.
- Accept common `-`, `*`, and `+` checkbox lists and report unsupported syntax
  with source locations.
- Assign stable identity to new tasks only during an approved transaction.
- Interpret title, checkbox, parent, tag, priority, and date changes explicitly.
- Treat deletion as ambiguous; never silently delete or cancel ledger items.
- Decide whether line order is meaningful and add a domain fact if it is.
- Detect duplicate IDs, conflicting metadata, missing parents, and cycles.
- Compare the manifest's base frontier with the current ledger and perform a
  three-way semantic comparison when stale.
- Provide plan, approval, atomic apply, and deterministic refresh stages.

## Current assessment

Route C appears most compatible with the ledger. A `TODO/TODO.md` plus detail
files is worth testing as one human-facing layout. This work is intentionally
deferred until real single-ledger dogfooding clarifies which edits matter most.


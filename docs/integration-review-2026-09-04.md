# Integration Review: PromiseGrid and todo-report

Date: 2026-09-04
Status: review note; it records findings, not protocol decisions

## Purpose

This note records lessons from the current private `cdint-grid` repository and
the public `computerscienceiscool/todo-report` project. The todo ledger remains
the authority for its own decisions. Upstream experiments and implementations
are evidence, not silently imported requirements.

## PromiseGrid findings

The current private Grid implementation has a useful generic envelope boundary:

```text
grid([42(pCID), ...protocol-owned slots])
```

The outer tag is `1735551332`. A new complete Grid envelope is identified with
the `grid-cbor/0x1027` multicodec, while a launch pCID may identify exact
approved Markdown specification bytes with `raw/0x55`. Generic framing checks
CBOR structure, slot zero, UTF-8, safe bounds, and all visible tag-42 CIDv1
links. The selected pCID owns slot count, deterministic representation, proof,
signature, semantic, and resource rules.

This creates two distinct acceptance questions:

1. Is the object framing-valid Grid-CBOR?
2. Does a known pCID handler consider it protocol-valid?

Unknown pCIDs may be stored and inspected without being interpreted or treated
as authorization. Exact bytes remain authoritative and parsed structures are
disposable views.

The todo ledger already emits the correct outer tag and a pCID in slot zero,
but `todo-core` currently computes both specification and event CIDs with
`raw/0x55`. Existing event CIDs must remain unchanged. A later protocol may use
separate raw specification CIDs and `grid-cbor/0x1027` event CIDs.

Other gaps exposed by the comparison:

- there is no generic Rust Grid-envelope parser or framing validator;
- materialized Turso views are described as rebuildable but cannot yet be
  recreated by replaying stored envelopes;
- CBOR export omits protocol-specification blocks and accepted-head metadata;
- export and history ordering use wall time rather than causal traversal;
- one global head and `actor_id = "local"` cannot distinguish concurrent
  writers or agent-owned timelines;
- content relationships stored as text are not visible tag-42 CID links; and
- the draft vault-sync design chooses one authoritative branch after conflict,
  while newer Grid work preserves concurrent evidence and uses explicit
  reconciliation.

The upstream pCID registry, universal dispatch, public Grid-CBOR adoption,
identity proofs, reference-set representation, and production CAS design remain
unsettled. They should not be copied as completed standards.

## todo-report findings

`todo-report` treats a selected Markdown TODO index and its Git history as the
source of truth. It is branch-aware and read-only. Its strongest reusable ideas
are:

- `detect` before interpretation, with explicit supported-dialect findings;
- lint findings with source file, line, stable code, and severity;
- age derived from durable history;
- branch drift as real context rather than noise;
- repository, monorepo, and fleet health summaries;
- text, Markdown, JSON, and TSV renderers over one report model;
- stable JSON snapshots and script-friendly exit codes;
- static daily HTML reports that require no application server; and
- partial-failure reporting, so one inaccessible repository does not hide the
  health of the rest.

Its Markdown support is broader than the todo ledger's importer: several
top-level ID conventions, checklist and table indexes, nested subtask IDs,
multiple authoritative indexes, and mixed-dialect detection. It also catches
index/detail-file disagreement, missing details, orphans, duplicate IDs, bad
checkboxes, and missing subtask references.

The products nevertheless have different authority models:

```text
todo-report: Markdown + Git -> read-only derived reports
todo ledger: immutable events -> mutable relational and Markdown views
```

Useful local successors would be `todo health`, `todo lint`, Markdown-dialect
detection, stable report JSON, meaningful exit codes, and a static global or
per-project dashboard. Remote repository scanning is less urgent because the
ledger already has a global cross-project view.

Running `todo-report` on this repository currently fails because it has no
`TODO/TODO.md`. Running it on `cdint-grid` reports a clean index with 36 open
and 10 completed top-level TODOs. This is evidence that its repository-oriented
workflow is usable, not a decision that this ledger must adopt its file layout.

## Editable Markdown direction

Editable Markdown is promising as an explicitly managed projection:

```text
event ledger -> manifest -> human edit -> semantic plan -> approval -> events
```

A safe flow would preserve non-task Markdown, identify existing items stably,
show an exact semantic diff, reject or merge stale edits, require approval, and
apply the resulting events atomically. Removing a line must not silently delete
or cancel an item. Reordering also has no durable meaning until the domain gains
an explicit sibling-order fact.

HTML comments are not selected. Alternatives include readable stable slugs,
Obsidian-style block IDs, explicit Markdown links, sidecar mappings, and a
`TODO/TODO.md` plus detail-file layout. These belong to TE-0009 and DR-0003.

## Candidate sequence

Implementation update: steps 1-3 are now far enough along for dogfooding.
DI-0008 introduces the generic Grid envelope boundary and v4 event identity;
DI-0009 introduces non-overwriting exact and CLI-restorable backups. Exact
archive replay remains deliberately separate from Markdown restoration.

1. Use the current CLI for one local global ledger and dogfood real projects.
2. Decide the Grid integration and implementation-language boundary.
3. Add generic framing validation and exact replay before synchronization.
4. Add managed Markdown planning only after its identity and authority model is
   settled.
5. Move from local project association to one-to-three repository interaction.
6. Add personal/shared spaces and multi-user timelines after identity,
   acceptance, and conflict semantics are decided.

# TE-0007: Machine-to-Machine Sync

Date: 2026-09-01
Status: selected

## Pressure

Dogfooding happens on more than one machine, but the ledger has no server and
none is wanted yet. The main Obsidian vault already syncs between devices, so
the cheapest transport is a file that rides along inside the vault. The ledger
must not corrupt under that transport, and the DI-0006 fidelity tiers must
stay intact: Markdown for humans, JSON for tools, CBOR as the durable record.

## Routes

### A: Sync the SQLite database file itself

The data directory joins the vault. Rejected: a live ledger is at least two
files (db + WAL), SQLite under a file syncer corrupts or silently branches,
and two devices writing between syncs leave no merge story. Events and views
would diverge together.

### B: Sync a Markdown or JSON view; re-import on the other side

Human-friendly, but the Markdown tier is intentionally lossy (DI-0006):
notes, citations, priority, and history would not survive a round trip, and
Markdown import mints new item IDs, so re-import duplicates everything.
Rejected as a sync record; remains useful as a human export.

### C: Sync the exact CBOR event archive with idempotent replay

`todo export cbor` already emits the exact immutable envelopes. An archive
import would insert only envelopes whose CID is absent and rebuild views from
events. Sound as an interchange snapshot, but a whole-file archive gives git
no line-level merge story; see route D for the selected refinement.

### D: Git as the transport

A private git remote gives versioned archives, visible conflicts, and real
merge semantics from any machine. Obsidian Sync skips `.git/`, so a repo
inside the vault cannot ride the vault sync; git needs its own remote.

## Resolution

The sync mechanism is Obsidian Sync, but as a per-file last-writer-wins mover
it can silently drop one side's events, and a single archive file gives no
merge story. Selected route D combined with an append-only log shape:

- The interchange artifact is an append-only JSONL log: one line per event
  envelope, carrying the CID and the exact envelope bytes (hex).
- Lines are canonically ordered by CID and deduplicated, so any union of the
  same event set produces identical file content.
- Git with the `merge=union` driver merges concurrent appends from two
  machines; replay deduplicates by CID, so convergence is automatic and
  lossless.
- Views are derived per event in canonical `(recorded_at, event_id)` order.
  A forked history after concurrent writes stays as a DAG in `parent_cid`;
  the head pointer becomes the newest event by the same ordering.
- SQLite stays local and never syncs; it remains a rebuildable view over the
  events (DI-0002). See DI-0007.

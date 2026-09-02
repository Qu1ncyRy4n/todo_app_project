# TE-0007: Vault-Piggyback Sync

Date: 2026-09-01
Status: draft

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

Selected. `todo export cbor` already emits the exact immutable envelopes.
Add `todo import cbor` that inserts only envelopes whose CID is absent,
verifies parent linkage, and rebuilds views from events. Views stay local and
rebuildable (AGENTS.md: exact events separate from views).

The wrapper loop for v0:

```text
pull:   copy vault archive aside, import cbor (CID dedup), rebuild views
work:   ordinary todo commands, events append locally
push:   export cbor to the vault archive, sync moves it
```

Failure mode to accept in v0: the ledger keeps one parent-linked chain and
one head. Two devices appending between syncs produce two heads, so push
refuses when the vault head differs from the last-synced head, and the human
resolves by choosing which side is authoritative. Single user, short windows:
acceptable. A multi-parent merge (the Grid envelope already carries a parents
array) is the later real fix and belongs to the PromiseGrid protocol family.

### D: Git as the transport

A bare repo or branch under the vault gives versioned archives and visible
conflicts. Compatible with route C (git is just the mover), but assumes the
vault is git-synced, which is not yet true here.

## Open questions before a DI

- What actually syncs the vault (Syncthing, Obsidian Sync, git, other)?
  Conflict visibility and atomicity differ per mechanism.
- Single-writer-between-syncs acceptable for dogfooding? (Assumed yes.)
- Plain CBOR in the vault, or encrypted at rest?
- Does `import cbor` need to verify the full parent chain before accepting an
  archive, or is CID membership enough for v0?
- Should push/pull be a native `todo sync` subcommand or a wrapper script
  until the semantics settle?

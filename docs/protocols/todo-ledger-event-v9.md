# Todo Ledger Event Protocol v9

Status: ledger-local protocol for explicit Git synchronization. It retains v8
and adds atomic scalar item patches.

The pCID is `CIDv1(raw/0x55, sha2-256(specification bytes))` over this file.
The envelope and deterministic ten-field payload are exactly as specified by
v5. Historical v1-v8 events remain valid and replayable.

`item.patched` has an item subject and may contain scalar `title`, `priority`,
`state`, `due_on`, and `defer_until` fields in data. Empty date values clear a
date. The sender validates all supplied values before emitting one event; the
projection applies the complete patch atomically. Relationships and append-only
notes remain explicit operations.

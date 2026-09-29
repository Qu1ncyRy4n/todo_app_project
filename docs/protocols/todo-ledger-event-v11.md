# Todo Ledger Event Protocol v11

Status: ledger-local protocol for explicit Git synchronization. It retains v10
and adds effort estimation fields.

The pCID is `CIDv1(raw/0x55, sha2-256(specification bytes))` over this file.
The envelope and deterministic ten-field payload are exactly as specified by
v5. Historical v1-v10 events remain valid and replayable.

`item.difficulty-changed` has an item subject and carries a `difficulty` field in
data holding an integer from 1 to 5, or an empty value to clear it. Difficulty is
a judgment of how hard the work is, independent of how long it takes.

`item.estimate-changed` has an item subject and carries an `estimate_minutes`
field in data holding a non-negative integer, or an empty value to clear it.

Both fields are optional and descriptive. Nothing in the ledger derives readiness,
ordering, or state from them; a reader that computes a weight from them does so in
its own presentation layer, so the weighting can change without a protocol change.

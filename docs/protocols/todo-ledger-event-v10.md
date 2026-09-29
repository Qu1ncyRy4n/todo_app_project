# Todo Ledger Event Protocol v10

Status: ledger-local protocol for explicit Git synchronization. It retains v9
and adds reversible item deletion.

The pCID is `CIDv1(raw/0x55, sha2-256(specification bytes))` over this file.
The envelope and deterministic ten-field payload are exactly as specified by
v5. Historical v1-v9 events remain valid and replayable.

`item.deleted` has an item subject and carries a `deleted_at` RFC 3339 timestamp
in data. The projection marks the item deleted; a deleted item is excluded from
every read that does not explicitly ask for deleted items, and its relationships
are excluded with it. The item's rows are retained so the deletion can be
reversed.

`item.restored` has an item subject and clears the deletion mark, returning the
item and its relationships to ordinary reads.

Deletion is a projection state, not ledger truth: the log remains append-only and
no historical event is removed or rewritten. A ledger-level erasure would be a
separate operation against the log itself and is not part of this protocol.

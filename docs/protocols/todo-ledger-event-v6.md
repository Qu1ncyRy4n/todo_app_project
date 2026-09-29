# Todo Ledger Event Protocol v6

Status: ledger-local protocol for explicit Git synchronization. It keeps the
v5 Grid-CBOR envelope and persistent actor identity, while distinguishing
planned dependencies from active blockers.

## Identity and envelope

The pCID is `CIDv1(raw/0x55, sha2-256(specification bytes))` over the exact
bytes of this file. A complete event is
`CIDv1(grid-cbor/0x1027, sha2-256(envelope bytes))`.

The envelope and deterministic ten-field payload are exactly as specified by
v5. Historical v1-v5 events and their CIDs remain valid and replayable.

## Event kinds

v6 permits every v5 event kind, plus:

- `item.block-added`
- `item.block-removed`

`item.dependency-added` and `item.dependency-removed` mean a planned
`depends_on` prerequisite. Their target uses `depends-on` and
`no-longer-depends-on` links. They affect readiness until the prerequisite is
done or canceled.

`item.block-added` and `item.block-removed` mean a current `blocked_by`
impediment. Their target uses `blocked-by` and `no-longer-blocked-by` links.
An item with any active blocker is not ready; the inverse `blocks` view is
derived, never separately written. Active blockers are currently item-to-item;
standalone issue records are intentionally deferred.

Structural parenthood remains an item field and never implies either relation.

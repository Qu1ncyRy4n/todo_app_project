# Todo Ledger Event Protocol v12

Status: ledger-local protocol for explicit Git synchronization. It retains v11
and adds typed cross-project relations.

The pCID is `CIDv1(raw/0x55, sha2-256(specification bytes))` over this file.
The envelope and deterministic ten-field payload are exactly as specified by
v5. Historical v1-v11 events remain valid and replayable.

`item.relation-added` has an item subject, a `relation` field in data, and a
`target` link holding the item the subject relates to. The relation names are
`contributes-to` and `implements`.

`item.relation-removed` has the same shape and withdraws one relation.

A typed relation is not structural parenthood and not a dependency. It may cross
project boundaries, it does not imply containment, and it never affects readiness:
an item is not blocked by anything it contributes to or implements. An item has at
most one relation of a given name to a given target, and may not relate to itself.

The inverse direction is derived when read, never stored twice, exactly as the
dependency and blocker views already are.

# Todo Ledger Event Protocol v7

Status: ledger-local protocol for explicit Git synchronization. It retains the
v6 Grid-CBOR envelope and relationship semantics, and adds ordered focus plans.

## Identity and envelope

The pCID is `CIDv1(raw/0x55, sha2-256(specification bytes))` over the exact
bytes of this file. The envelope and deterministic ten-field payload are
exactly as specified by v5. Historical v1-v6 events remain valid and
replayable.

## Focus-sequence events

- `focus-sequence.created`: subject is a UUID sequence identifier; data holds
  its non-empty `title`.
- `focus-sequence.entry-added`: subject is the sequence identifier; data holds
  a monotonically increasing `position`; link `item` targets an existing item.
- `focus-sequence.entry-removed`: subject is the sequence identifier; data
  holds `position`.

A focus sequence is an ordered, cognitive plan referencing existing tasks. It
does not change parentage, dependency readiness, lifecycle, or priority.

Every v6 event kind is also permitted by v7.

# Todo Ledger Event Protocol v2

Status: ledger-local protocol selected by DI-0005. It extends v1 for actionable
focus metadata without claiming PromiseGrid compatibility.

## Envelope and payload

Encoding is identical to Todo Ledger Event Protocol v1:

```text
grid([42(pCID), parents, payload, null])
```

The pCID is the CIDv1 raw SHA-256 CID of the exact bytes of this v2
specification. The deterministic payload has the same ten ordered keys defined
by v1. Proof remains `null`.

## v0.2 event kinds

- `item.priority-changed`
- `item.due-date-changed`
- `item.defer-date-changed`
- `item.dependency-added`
- `item.dependency-removed`

Existing v1 event kinds continue to use the v1 pCID. Consumers may therefore
resolve old events against the frozen v1 bytes while recognizing v0.2 focus
events through this specification.

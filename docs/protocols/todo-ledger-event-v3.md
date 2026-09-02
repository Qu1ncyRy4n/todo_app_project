# Todo Ledger Event Protocol v3

Status: ledger-local protocol selected by DI-0006. It extends prior protocols
for append-only correction events without claiming PromiseGrid compatibility.

## Envelope and payload

Encoding is identical to Todo Ledger Event Protocol v1:

```text
grid([42(pCID), parents, payload, null])
```

The pCID is the CIDv1 raw SHA-256 CID of the exact bytes of this v3
specification. The deterministic payload has the same ten ordered keys defined
by v1. Proof remains `null`.

## v0.2.1 event kinds

- `item.title-changed`
- `item.parent-cleared`
- `item.project-changed`
- `namespace.removed`

Existing event kinds retain the pCID of the protocol version that introduced
them.

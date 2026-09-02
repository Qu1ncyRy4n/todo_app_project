# Todo Ledger Event Protocol v1

Status: project-local protocol selected by DI-0002. It is not a PromiseGrid
protocol and makes no compatibility claim.

## Envelope

Events use Grid-CBOR framing:

```text
grid([42(pCID), parents, payload, null])
```

- `grid` is CBOR tag 1735551332.
- `pCID` is the CIDv1 raw SHA-256 CID of the exact bytes of this specification.
- `parents` is an array containing zero or one prior event CID in v0.1.
- `payload` is the deterministic map below.
- proof is `null` until a later identity TE selects signing semantics.

## Payload

Keys are encoded in the order shown:

1. `schema_version`: unsigned integer `1`.
2. `action`: text `record`.
3. `kind`: versioned event-kind text.
4. `event_id`: UUID text used for CLI and database lookup.
5. `actor_id`: text; v0.1 uses `local`.
6. `subject_id`: UUID text.
7. `project_id`: UUID text or null.
8. `recorded_at`: RFC 3339 UTC text.
9. `data`: deterministic text-to-text map ordered by key.
10. `links`: ordered array of `{relation, target}` maps.

The event CID is the CIDv1 raw SHA-256 CID of the complete encoded envelope.
Wall-clock timestamps are descriptive. Parent CIDs carry event order.

## v0.1 event kinds

- `item.created`
- `item.state-changed`
- `item.note-added`
- `item.citation-added`
- `item.parent-set`
- `namespace.created`
- `namespace.renamed`
- `namespace.assigned`
- `project.path-attached`

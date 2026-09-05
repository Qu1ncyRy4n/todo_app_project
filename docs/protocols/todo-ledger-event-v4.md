# Todo Ledger Event Protocol v4

Status: ledger-local protocol selected by DI-0008. It adopts the private
launch-minimal Grid-CBOR framing contract without claiming public PromiseGrid
protocol compatibility.

## Identity

The pCID is `CIDv1(raw/0x55, sha2-256(specification bytes))` over the exact bytes
of this file. A complete event is identified by
`CIDv1(grid-cbor/0x1027, sha2-256(envelope bytes))`.

Historical v1-v3 events and their raw-codec CIDs remain unchanged. A v4 parent
link may therefore target either a legacy raw CID or a v4 Grid-CBOR CID.

## Envelope

The exact pCID-owned shape is:

```text
grid([42(pCID), parents, payload, proof])
```

- `grid` is CBOR tag `1735551332`.
- `parents` is a definite array containing zero or one tag-42 event CID.
- `payload` is the deterministic map below.
- `proof` is `null` in v4.

The outer envelope must also satisfy the generic framing validation selected by
DI-0008. This protocol requires exactly three slots after the pCID.

## Payload

The payload is a definite map with these ten keys in this exact order:

1. `schema_version`: unsigned integer `1`.
2. `action`: text `record`.
3. `kind`: one event-kind string listed below.
4. `event_id`: canonical hyphenated UUID text.
5. `actor_id`: text; v4 uses `local`.
6. `subject_id`: text identifying the affected domain subject.
7. `project_id`: canonical UUID text or `null`.
8. `recorded_at`: RFC 3339 UTC text.
9. `data`: a definite text-to-text map ordered lexicographically by key.
10. `links`: a definite array of maps. Each map has `relation` and `target`
    text keys in that order.

The representation is protocol-deterministic but is not declared to be
DAG-CBOR. Text must be valid UTF-8. Duplicate payload or data keys are invalid.
Receivers should apply explicit input-size and nesting limits before semantic
projection.

## Event kinds

- `item.created`
- `item.state-changed`
- `item.note-added`
- `item.citation-added`
- `item.parent-set`
- `item.title-changed`
- `item.parent-cleared`
- `item.project-changed`
- `item.priority-changed`
- `item.due-date-changed`
- `item.defer-date-changed`
- `item.dependency-added`
- `item.dependency-removed`
- `namespace.created`
- `namespace.renamed`
- `namespace.assigned`
- `namespace.removed`

`project.path-attached` is not included. Filesystem attachment is machine-local
configuration and has never been emitted as a durable ledger event.

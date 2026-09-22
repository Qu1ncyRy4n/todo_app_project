# TE-0010: Grid-CBOR Compatibility Without Legacy Relabeling

Date: 2026-09-04
Status: selected

## Pressure

The ledger already stores exact Grid-shaped CBOR envelopes, but identifies both
protocol specifications and complete envelopes with the `raw/0x55` multicodec.
The current private Grid definition uses `raw/0x55` for exact Markdown pCID
specifications and `grid-cbor/0x1027` for complete Grid-CBOR blocks. Existing
ledger events may already be durable user evidence and cannot be relabeled.

## Routes

### A: Relabel all existing events as Grid-CBOR

The envelope bytes and multihash would stay the same, but every CID string would
change. Parent links, database heads, exported archives, and external references
would no longer name the original objects. Rejected.

### B: Keep emitting raw event CIDs forever

This preserves history but prevents new objects from making the selected
Grid-CBOR interpretation explicit. A sidecar would require todo-specific
special cases before generic Grid storage or traversal. Rejected.

### C: Preserve v1-v3 and emit a v4 successor

Keep every stored legacy byte and CID unchanged. New events use a standalone v4
pCID specification, the same four pCID-owned slots, and a
`CIDv1(grid-cbor/0x1027, sha2-256(envelope bytes))` event CID. A v4 event may
visibly link to a raw-codec legacy parent because Grid links require CIDv1, not
one universal target multicodec. Selected.

## Failure and concurrency

Building and v4 replay validate generic framing before an event is accepted.
The replay path recognizes only the v4 pCID and retains v1-v3 records without
reinterpreting them, so it does not silently accept unknown pCIDs or relabel
legacy evidence. The store continues to serialize one local writer through its
existing transaction and head until a later timeline decision supersedes that
model.

## Migration and scale

No database rewrite is required. Existing rows keep their CID, parent CID, and
envelope. New rows form a compatible successor chain. Generic parsing uses
input-bounded iterative scanning so deeply nested CBOR does not consume the Rust
call stack. Receiver or pCID-specific admission limits remain above framing
validity.

## Decision

Select route C. Add a protocol-neutral Rust Grid envelope module, verify it
against the private cross-language conformance vectors, and make all newly
created todo events v4 Grid-CBOR objects. Do not claim public PromiseGrid
conformance while public adoption remains open.

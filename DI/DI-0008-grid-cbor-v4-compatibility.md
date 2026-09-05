# DI-0008: Grid-CBOR v4 Compatibility

Date: 2026-09-04
State: accepted
Source: TE-0010

Preserve every v1-v3 event envelope and its original `raw/0x55` CID. All newly
built events use the standalone todo event protocol v4 pCID and identify their
complete envelope bytes with `grid-cbor/0x1027` plus SHA-256.

Implement generic Grid framing independently of todo semantics. It validates
one complete CBOR item, the registered Grid tag, an array with a tag-42 CIDv1
pCID in slot zero, UTF-8 text, safe item boundaries, and every recursively
visible tag-42 CIDv1. It preserves exact protocol-owned slot bytes and does not
assign them meaning, canonicalize them, impose proof rules, or authorize an
application action.

The todo v4 handler owns the four-slot parents/payload/proof shape and its
deterministic representation. Unknown pCIDs may later be retained as
framing-valid evidence but must not be projected without a known handler.


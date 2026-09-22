# Todo ledger event log v1

Transport format for machine-to-machine sync (TE-0007, DI-0007). A sync
repository contains `ledger.toml`, which identifies this format as version 1,
and one UTF-8 file named `events.jsonl`. The log is an append-only record of
exact event envelopes; it carries no views.

## Line format

Each non-empty line is one JSON object with exactly two string fields:

```json
{"cid": "<CIDv1 base32 of the envelope bytes>", "bytes": "<lowercase hex of the exact envelope bytes>"}
```

## Rules

1. `bytes` are the exact event envelope bytes identified by `cid`. Historical
   v1-v3 events use `CIDv1(raw/0x55, sha2-256(bytes))`; v4-v5 events use
   `CIDv1(grid-cbor/0x1027, sha2-256(bytes))`. Receivers must recompute the
   CID using its declared codec and reject mismatches.
2. Lines are canonically ordered by the `cid` string and duplicate CIDs
   appear at most once. Any union of the same event set therefore has one
   canonical file content.
3. Empty lines are ignored. The file ends with a newline.
4. CIDs already known to a receiver are skipped; replayable events are applied
   in canonical `(recorded_at, event_id)` order and derive local views. The
   first sync implementation replays v5 only; it preserves but does not
   reinterpret historical v1-v4 records pending a migration handler.
5. The parent chain may fork after concurrent writes. Forks are preserved in
   the envelopes; consumers resolve the current head as the event with the
   greatest `(recorded_at, event_id)`.

## Git transport

The file lives in its own repository with `.gitattributes` containing
`events.jsonl merge=union`. Concurrent appends merge by concatenation; the
next writer rewrites the file canonically. Replay is idempotent by CID.

# Todo Ledger Event Protocol v8

Status: ledger-local protocol for explicit Git synchronization. It retains v7
and adds append-only task time sessions.

## Identity and envelope

The pCID is `CIDv1(raw/0x55, sha2-256(specification bytes))` over this file.
The envelope and deterministic ten-field payload are exactly as specified by
v5. Historical v1-v7 events remain valid and replayable.

## Session events

- `item.session-started`: subject is the task item. Data holds canonical UUID
  `session_id` and RFC 3339 UTC `started_at`.
- `item.session-finished`: subject is the task item. Data holds the same
  `session_id` and RFC 3339 UTC `finished_at`.

Sessions are append-only execution records. A task has at most one unfinished
session at a time; finishing requires a time no earlier than its start.
Every v7 event kind is also permitted by v8.

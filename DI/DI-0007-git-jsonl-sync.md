# DI-0007: Git-Transported JSONL Event Log Sync

Date: 2026-09-01
Status: active
Source: TE-0007

Decision: Sync machines through a private git repository holding one
append-only JSONL file. Each line carries an event CID and the exact Grid-CBOR
envelope bytes in hex. Lines are canonically ordered and deduplicated by CID,
and git's `merge=union` driver merges concurrent appends, so replaying the
union of event sets converges without a single-writer discipline.

Intent: Keep the durable record exact and content-addressed (DI-0002) while
making the local SQLite database a device-private rebuildable view. Views are
derived per event in `(recorded_at, event_id)` order; a forked parent chain
after concurrent writes is tolerated as a DAG and the head resolves to the
newest event under the same ordering. The format is specified in
`docs/protocols/todo-ledger-log-v1.md`.

Consequences: SQLite is never a sync artifact. Same-item concurrent edits
resolve by canonical event order, which single-user dogfooding accepts.
Multi-parent merge and observer-based assessment remain future protocol
families.

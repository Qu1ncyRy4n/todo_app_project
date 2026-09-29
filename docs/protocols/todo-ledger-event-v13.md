# Todo Ledger Event Protocol v13

Status: ledger-local protocol for explicit Git synchronization. It retains v12
and adds the `void` item state.

The pCID is `CIDv1(raw/0x55, sha2-256(specification bytes))` over this file.
The envelope and deterministic ten-field payload are exactly as specified by
v5. Historical v1-v12 events remain valid and replayable.

`item.state-changed` may carry `void` as a state. A void item was created in
error: a duplicate, a mistyped capture, a line that was never a task. It is
excluded from every read that does not ask for void items specifically, including
listings that ask for all states, because "all states" means all the work, and a
void item was never work.

Void is distinct from canceled. A canceled task was real work that was
deliberately abandoned, and belongs in history. A void item is noise, and its
only remaining purpose is that it can be brought back if it was voided by
mistake.

Any state may move to void, and void may move back to inbox or active. Voiding is
therefore always reversible, and nothing is erased: the log keeps every event, as
it does for deletion.

# Todo Ledger Event Protocol v15

Status: ledger-local protocol for explicit Git synchronization. It retains v14
and adds an agent signature on any event.

The pCID is `CIDv1(raw/0x55, sha2-256(specification bytes))` over this file.
The envelope and deterministic ten-field payload are exactly as specified by
v5. Historical v1-v14 events remain valid and replayable.

Any event may carry an `agent` field in data. Its value names the agent that
wrote the event, such as `agent` or `review-bot`, in the namespace segment
alphabet. An event without it was written by the operator of the device named
by `actor_id`. The field is a claim made by the writing process, not proof: it
lets agent work be filtered, and authorizes nothing.

An item is an agent item when its `item.created` event carries `agent`. Later
events on the item keep their own signatures; they do not change who created
it.

`actor_id` keeps its v5 meaning, a persistent per-device identifier, for both
signed and unsigned events.

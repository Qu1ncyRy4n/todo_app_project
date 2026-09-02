# TE-0002: Event Envelope And Ordering

Status: concluded by DI-0002.

## Decision under test

Should v0.1 use mutable rows, application JSON events, or Grid-CBOR framing?

## Alternatives

- A: mutable task rows. Smallest implementation, but loses history and creates
  future synchronization conflicts.
- B: application-specific JSON events. Portable, but requires a later wire
  migration and has no exact protocol selection.
- C: project-local Grid-CBOR events with pCID, parents, payload, and proof
  slots. More initial work, but preserves exact records and a path toward PG.

## Scenarios

On crash recovery, C can rebuild views from exact accepted events. Concurrent
future actors can retain separate parent histories without overwriting rows.
Unknown future pCIDs can be retained without interpretation. Changing identity
rules does not require pretending unsigned v0.1 events were authenticated.

## Conclusion

Select C with a null proof slot and an explicit non-PG compatibility statement.
One parent orders the local timeline; typed payload links express other
relationships.


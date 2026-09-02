# TE-0001: Capture And Commitment

Status: concluded by DI-0001.

## Decision under test

Should capture immediately create a Promise-Theory commitment?

## Alternatives

- A: every todo is a promise. Simple metrics, but an inbox becomes a collection
  of broken commitments.
- B: capture first; activate explicitly. Cheap capture remains psychologically
  safe and `active`/`ongoing` identifies deliberately selected work.
- C: expose every record as a generic PromiseGrid promise. Semantically compact
  at the wire layer, but too formal for current personal workflow.

## Scenarios

Under rapid capture, A discourages recording ideas. Under overdue work, A mixes
neglected possibilities with actual commitments. Under later PromiseGrid
integration, B can map activation or an explicit future `promise` operation to
a promise without changing capture. Under migration, B preserves the useful
distinction even if its vocabulary evolves.

## Conclusion

Select B. Use `inbox`, `active` (alias `ongoing`), `done`, and `canceled` in
v0.1. Do not claim that `active` is already a formal PromiseGrid promise.


# TE-0004: Completion And Assessment

Status: concluded by DI-0004.

## Decision under test

Should completion be a global boolean, a local explicit assessment, or an
automated evidence policy?

## Alternatives

- A: global `done` boolean. Convenient but destroys provenance and cannot
  represent disagreement.
- B: append local lifecycle and assessment records. Honest and usable now.
- C: automatically assess from versioned evidence policies. Useful for agents,
  but premature before workflows supply real policy cases.

## Conclusion

Select B now and C later. v0.1 exposes ordinary lifecycle commands rather than
the full Promise-Theory vocabulary. `done` appends a local record; it does not
rewrite the creation event. More formal observations and assessments require a
later protocol.


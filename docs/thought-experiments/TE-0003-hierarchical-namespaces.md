# TE-0003: Hierarchical Namespaces

Status: concluded by DI-0003.

## Decision under test

Should classification use flat tags, path strings, or stable namespace nodes?

## Alternatives

- A: flat tags. Easy but ambiguous at scale.
- B: hierarchical path strings. Good browsing, but renames invalidate or
  rewrite assignments.
- C: stable nodes with kind, canonical path, aliases, and assignment by ID.

## Scenarios

Prefix search is natural under B and C. A project rename under B requires row
rewrites; under C it creates a rename event while old paths remain aliases.
Cross-project use benefits from typed roots such as `project/`, `role/`,
`goal/`, `topic/`, and `context/`. Arbitrary multi-parent taxonomy would make
tree display and inheritance ambiguous, so it is deferred.

## Conclusion

Select C. Use one canonical parent, aliases, descendant-inclusive prefix
queries, and an exact-match option. Extend through registered data schemas, not
runtime plugins in v0.1.


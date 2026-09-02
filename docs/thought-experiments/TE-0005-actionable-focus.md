# TE-0005: What Makes an Item Actionable?

Date: 2026-08-28
Status: selected

## Pressure

A well-organized tree can still present dozens of choices. v0.2 must reduce
arbitration without hiding why one item is recommended.

## Route A: One opaque score

Store or calculate a numeric score from all metadata. This is compact but hard
to audit, easy to overfit, and gives plugins or LLMs too much invisible policy.

## Route B: Explicit readiness plus lexicographic ranking

An item is ready when it is open, its defer date has arrived, and every
explicit blocker is complete. Rank ready items by visible fields in this order:

```text
active before inbox
-> critical, high, normal, low priority
-> dated before undated; earlier due date first
-> older item first
```

Return the fields that caused the result so `next --explain` is deterministic.

## Route C: LLM selection

Send the candidate set and context to a model. This can be a later advisory
view, but it is not durable or predictable enough to define readiness.

## Decision

Select B for v0.2. Preserve C as an optional future advisor whose suggestion
and rationale can be recorded as an observation.

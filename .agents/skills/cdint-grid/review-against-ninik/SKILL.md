---
name: review-against-ninik
description: Review a proposed design or implementation plan against the Ninik PromiseGrid design touchstone before code generation. Use when classifying design effect, identifying simpler existing mechanisms, checking Promise Theory alignment or hidden durable authority, validating claimed exceptions or disposable views, and deciding whether a departure needs an explicit user decision and DI.
---
<!--
TBD: Imported verbatim from captured cdint-grid skill review-against-ninik for QMR dogfooding.
This procedure may require cdint-grid executables, worker branches, identity rules,
coordination files, or organization decision records that are not configured in a
QMR consumer. Do not treat this import as approval or portable default policy.
-->

# Review Against Ninik

Read `docs/DN-ninik-promisegrid-design-philosophy-review.md` before reviewing.
If a worker tree predates that path, use only the exact note bytes named by its
handoff after the named confirmer supplies the commit, path, and SHA-256 and the
worker verifies them. Never substitute a summary or unverified copy. Source:
DI-nutan; DI-bihud

## Timing

For non-trivial work that still needs final DF questions:

1. Complete any required neutral TE before the first substantive Ninik review.
   Ninik may identify a missing scenario, but it does not choose among TE
   alternatives, answer a DF question, or lock a decision.
2. Review the surviving alternatives, proposed question context, and proposed
   final DF questions before asking the first question. A required correction
   blocks the DF sequence until it is incorporated and the affected material is
   reviewed again. Incorporate a recommended correction or explicitly
   disposition it, and keep the recommendation and disposition visible in the
   question context.
3. If an earlier DF answer materially changes a later question or its surviving
   alternatives, review that affected question again before asking it.
4. After the answers and DIs, review only the decision-driven changes in the
   final implementation plan. Rerun the affected full review when the selected
   design was not reviewed earlier or materially differs from the reviewed
   alternatives. When the answers select reviewed alternatives without changing
   the plan, record that no decision-driven change exists instead of repeating
   the full review. User approval of the reviewed final plan releases code
   generation.

Work that needs no DF receives the ordinary final pre-code review. Do not add a
post-code review and present it as though it governed earlier questions or
implementation. Source: DI-torut; DI-latag; DI-gafom

## Review

Perform this review before code generation. A later result may record this
review, its approval, and any implementation deviation, but must not present a
new post-code review as though it governed the earlier implementation. Source:
DI-gafom

1. Identify the exact proposed design, final DF question set, or implementation
   plan and governing decisions. Separate settled decisions, proposals,
   experiments, and unresolved questions.
2. Decide whether the work affects product or coordination design. Purely
   mechanical changes with no design effect need no invented architecture
   analysis.
3. For design-affecting work, apply the touchstone:
   - Keep durable promises and source evidence distinct from mutable local
     decisions, rebuildable views, and external effects.
   - Prefer the minimal PromiseGrid model and an existing mechanism. Treat the
     default top-level semantic action as `promise`; pCIDs define specialized
     meaning.
   - Preserve named promisers, local acceptance, exact source and effect links,
     external readback, and correction rather than overwrite.
   - Require a claimed disposable view to name its frontier, derivation context,
     recovery source, and demonstrated rebuild behavior.
   - Preserve necessary specialized mechanics for external adapters, bootstrap,
     queues and checkpoints, secrets, physical indexes, balanced accounting,
     and bounded experiment or coordination evidence without promoting them to
     hidden product authority.
   - State current decision status and unresolved questions; do not present a
     recommendation or analogy as a locked decision.
4. Inventory every applicable existing primitive, reusable pattern, or local
   mechanism. For each one, state how the current plan already uses, omits, or
   conflicts with it. Do not invent a mechanism when none applies.
5. Select exactly one classification:
   - `no design effect` — the work changes no product or coordination design;
   - `alignment` — it follows the touchstone and needs no departure;
   - `approved departure` — an explicit governing user decision and DI approve
     the departure; or
   - `unresolved departure` — stop and obtain a user decision and DI before the
     work proceeds.

## Report

Return a concise `Ninik Review` with these fields:

- `Classification:` one exact classification above.
- `Touchstone:` the applicable section or pattern.
- `Rationale:` the concrete alignment or departure.
- `Decision status:` cite the governing DI for an approved departure, identify
  the required decision for an unresolved departure, or state that none is
  needed.

Then emit exactly one top-level Markdown bullet for every applicable existing
mechanism. Give each finding these six separate nested bullets in this order:

```markdown
- Mechanism: <mechanism name>
  - Current treatment: <how the plan uses, omits, or conflicts with it>
  - Severity: <required, recommended, or none>
  - Plan location: <exact plan location>
  - Proposed edit: <concrete edit or none>
  - Disposition: <visible disposition>
  - Reason: <Ninik-based reason>
```

Start `Mechanism` at the top list level. Keep the six child bullets contiguous
and nested beneath it; do not combine them into a wrapped paragraph or introduce
them with an outer `Mechanisms` bullet. Continuation prose remains within its
own child bullet. Do not repeat a mechanism in separate findings. When one
mechanism supports multiple corrections, pair every severity with its exact plan
location, edit, and disposition inside the corresponding child values.

Use severity `required` or `recommended` for a correction and `none` when no
correction is needed. A required correction remains `pending` and blocks
progress until incorporated and reviewed again. A recommended correction
records its visible disposition. An already-correct treatment uses proposed edit
`none` and disposition `already satisfied`.

When no existing mechanism applies, emit one finding with `Mechanism: no
applicable existing mechanism` and the same six nested bullets. Name the reviewed
plan scope in `Plan location`, use severity `none`, proposed edit `none`,
disposition `not applicable`, and explain why in `Reason`. Do not emit standalone
`Simpler existing mechanism` or `Plan changes` fields. Source: DI-zaniv;
DI-pataf

The Ninik note, its source-fidelity promotion, and this skill do not review
themselves. Validate those artifacts through exact bytes and hashes, functional
tests, and bounded main acceptance unless the repo owner explicitly approves a
separate review for one concrete-harm case. Apply this review normally to
non-Ninik design portions of mixed work. Source: DI-bihud; DI-gafom

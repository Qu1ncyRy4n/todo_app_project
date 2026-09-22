---
name: handback
description: Review and repair results.md for a historical or still-active old-style worker attempt. Use only for legacy attempts governed by handoff packets, or when the user explicitly requests compatibility handback; new peer workers publish Git promise commits instead.
---
<!--
TBD: Imported verbatim from captured cdint-grid skill handback for QMR dogfooding.
This procedure may require cdint-grid executables, worker branches, identity rules,
coordination files, or organization decision records that are not configured in a
QMR consumer. Do not treat this import as approval or portable default policy.
-->

# Handback

Make one legacy worker result self-contained, accurate, and usable without
repeating settled work. Reconcile complete artifacts rather than their
summaries. New peer work does not create `results.md`; peers consume its Git
promise commits through the `consensus` skill. Source: DI-zupos; DI-pufuk

## Review

1. Identify the exact `results.md`, authoritative `handoff.json` when present,
   linked original `handoff.md`, associated
   `merge-checks.json`, retained run evidence, candidate commits, and relevant
   session log or current conversation. State any unavailable artifact; never
   imply that unavailable context was checked.
2. Read every identified artifact in full. For a tracked attempt, strictly
   validate `handoff.json`, verify that `handoff.md` contains
   `Assignment metadata: [handoff.json](handoff.json)` near its top, and use the
   JSON rather than free-form Markdown as assignment authority. Distinguish the
   version-2 assignment-start `worker_base_commit` from the older shared
   `merge_base_commit`, and verify `merge_base_commit -> worker_base_commit ->
   worker tip`. For version 1, retain the assignment-start meaning of
   `worker_base_commit`; state the merge base as unknown unless separate queue
   evidence proves it. Source: DI-barir. For a legacy
   `/tmp` attempt without JSON, retain the exact fixed `Worker Identity`
   matching contract. Map every handoff requirement to an
   explicit result outcome and verify every named path, commit, test, decision,
   limitation, and follow-up that can be checked locally. Source: DI-zopaz
3. Compare `results.md` with the available session discussion. Capture every
   material decision, answered question, rejected alternative, correction,
   unresolved question, and next step that is not already present.
4. Repair `results.md` in place when the current worker owns that path. When it
   does not, produce an explicit blocked finding naming the required owner and
   durable destination; do not silently leave the omission for the receiver.
5. Keep mutually exclusive alternatives and intentionally deferred work as
   explicit dispositions. Do not blindly implement every recommendation or
   claim that a temporary report is durable project state.

## Acceptance Gates

Apply these gates before reporting `Handback: PASS`:

1. Require the handoff to contain its assignment-design `Ninik Review`. For work
   that still needed final DF questions, require the result to record any
   required neutral TE before the first substantive review and the pre-question
   Ninik classification, touchstone, rationale, and decision status. Require
   exactly one top-level finding for every applicable existing mechanism:

   ```markdown
   - Mechanism: <mechanism name>
     - Current treatment: <how the plan uses, omits, or conflicts with it>
     - Severity: <required, recommended, or none>
     - Plan location: <exact plan location>
     - Proposed edit: <concrete edit or none>
     - Disposition: <visible disposition>
     - Reason: <Ninik-based reason>
   ```

   Require the six child bullets to be separate, contiguous, and in that order.
   Reject a wrapped paragraph, an outer `Mechanisms` bullet, missing, extra,
   renamed, or misordered fields, and duplicate mechanism findings. One
   mechanism's multiple corrections remain in its single finding with every
   severity, location, edit, and disposition preserved in the corresponding
   child values. An already-correct treatment uses proposed edit `none` and
   disposition `already satisfied`; when none applies, require `Mechanism: no
   applicable existing mechanism` with the same six nested bullets. Do not
   accept standalone `Simpler existing mechanism` or `Plan changes` fields.
   Required corrections must have been incorporated and the affected material
   reviewed again before questioning continued. Recommended corrections and their
   explicit dispositions must remain visible. Require any affected later
   question to have been reviewed again after a materially changing earlier
   answer.

   After DF answers and DIs, require the result to record the review of the
   decision-driven final-plan changes, or the affected full review when the
   selected design was unreviewed or materially different. When the decisions
   left the reviewed plan unchanged, require that explicit record instead of a
   repeated full review. Require user approval before code generation. Work that
   needed no DF records the ordinary final pre-code review. Every result lists
   later implementation deviations or states that none occurred. Do not perform
   or require a new post-code Ninik design review. Source: DI-nutan; DI-bihud;
   DI-torut; DI-latag; DI-zaniv; DI-pataf; DI-gafom
2. Classify the handoff's barrier statement. When it explicitly says no
   coordination barrier applies, require no barrier chronology. Do not invent
   one from test cases, ordinary dependencies, or prose that merely discusses
   barriers.
3. When a barrier applies, require its allowed pre-barrier work, exact stop
   point, prerequisite, acceptable evidence, named confirmer, local
   verification, and failure-or-change behavior. Then require the result to
   record the worker's stop, exact confirmation from that named confirmer,
   local verification, and resumption of the same unfinished attempt.
4. Compare the result with the handoff rather than accepting chronology in
   isolation. Reject a wrong confirmer, missing or unverifiable evidence, or a
   change to scope, approved paths, assignment start, merge base, prerequisite,
   or confirmer. A
   governing change requires a fresh numbered attempt.
5. If available evidence shows that work crossed the stop point early, record
   the noncompliance, stop, and require a fresh attempt. Later confirmation
   does not cure known early crossing. Never claim that ordinary chronology
   proves a trusted worker could not have crossed early.
6. Do not require separate independent review. Normal acceptance uses worker
   self-review, deterministic merge-queue checks, and bounded main acceptance.
   Recognize a separate review only when the record identifies the exact
   candidate, concrete harm, scope, reviewer, expected delay, and explicit
   per-case user approval. Source: DI-gafom
7. State every failure in plain English: name the missing or conflicting
   evidence, the affected requirement, and the exact corrective action.
8. Do not classify a local artifact matched by a tracked repository
   `.gitignore` as a path-compliance failure. Verify the match and tracked ignore
   source, confirm that the artifact remains untracked, and continue to apply
   independent secret, production-data, side-effect, disk-use, privacy,
   retention, cleanup, and live-system rules. Source: DI-sohig

Barrier acceptance is trusted-worker coordination, not a semaphore, polling
loop, queue field, handoff schema field, validator service, or wire protocol.
Source: DI-gumiv; DI-rufim

## Required Result Content

Ensure the reviewed `results.md` contains:

- the exact assignment, assignment-start commit, merge-base commit, tip,
  changed paths, and retained evidence paths (with version-1 unknowns stated
  rather than inferred);
- a requirement-to-outcome matrix covering the complete handoff;
- decisions and rationale settled during the session;
- checks run, exact outcomes, and any checks not run;
- the handoff Ninik disposition; any required neutral TE; the pre-question
  Ninik review and mechanism-bullet dispositions; DF answers, DIs, and affected
  later-question reviews; the post-DI plan review and user approval; and the
  implementation-deviation disposition;
- the applicable no-barrier or complete-barrier outcome;
- findings, limitations, rejected alternatives, deferred work, and unresolved
  questions with their owners or durable TODO/DR destinations;
- enough concrete continuation steps for a new worker to proceed without
  reopening settled questions; and
- this instruction to the receiving session: read the entire result, address
  every substantive item, and persist each item in the repository as an
  implementation, verified existing record, owned TODO/DR, or explicit
  rejection, supersedence, or deferral. Do not integrate only the summary.

For a new tracked attempt, do not add or require the legacy fixed `Worker
Identity` section in `results.md`; those fields belong in adjacent
`handoff.json`. Do not remove that section from an older `/tmp` result because
legacy exact-identity discovery still depends on it.

## Verdict

Report `Handback: PASS` only when the result is complete and accurate against
all available artifacts and context and every acceptance gate above passes.
Otherwise report `Handback: BLOCKED`, name each missing or inaccurate item, and
state the exact action needed. A passing handback is still a report: it does not
replace worker self-review, merge-queue checks, bounded main acceptance, or
durable repository records. A separate review exists only after explicit
per-case user approval. Source: DI-gafom

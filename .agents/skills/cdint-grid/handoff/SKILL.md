---
name: handoff
description: Create or verify a legacy Codex worker handoff packet for a historical or still-active old-style attempt. Use only when an existing attempt remains governed by handoff.json and handoff.md; do not use this skill for new peer work, newtree, or ordinary continuation after peer-workflow activation.
---
<!--
TBD: Imported verbatim from captured cdint-grid skill handoff for QMR dogfooding.
This procedure may require cdint-grid executables, worker branches, identity rules,
coordination files, or organization decision records that are not configured in a
QMR consumer. Do not treat this import as approval or portable default policy.
-->

# Handoff

Use this compatibility skill only for a historical or still-active old-style
attempt. New peer work uses a governing TODO or DR, `newtree`, and Git promise
commits instead. `handoff.json` remains the authoritative machine assignment;
`handoff.md` is its adjacent human explanation and must link to it. This skill
does not grant authority beyond those records. Source: DI-jagol; DI-pugid;
DI-pufuk

## Create a handoff

1. Create the strict `handoff.json` metadata in the owning TODO attempt
   directory. Record the task and owning TODO, attempt and scratch paths,
   worktree, branch, pinned assignment-start `worker_base_commit`, older shared
   Git `merge_base_commit`, worker type, tmux session, and any evidence-backed
   thread identity. New records use schema version 2. Preserve a strict
   version-1 record unchanged when verifying historical evidence; in that
   schema its `worker_base_commit` retains the assignment-start meaning and the
   older merge base must come from separate queue evidence.
   Source: DI-barir
2. Run `cdint-grid-token-efficiency agents manifest` and record its Git commit and
   `AGENTS.md` blob in `handoff.md`.
3. State the exact objective, success conditions, writable and protected repo
   paths, approved runtime paths, locked decisions, unresolved categories,
   exclusions, checks, result path, and stop rules. Skills do not widen any of
   these boundaries. Local artifacts matched by a tracked repository
   `.gitignore` inherit the standing path approval in DI-sohig and need not be
   enumerated individually. Verify the match and tracked ignore source before
   use, keep each artifact untracked, and retain any operationally useful
   location or lifecycle guidance.
4. Load governing context with exact deterministic commands such as
   `todo --source PATH --task TASK`, `todo --source PATH --decision DI-ID`, and
   `glossary --term TERM`. Expand omitted context explicitly when it becomes
   relevant.
5. Include a concise `Ninik Review` that classifies the assigned work as no
   design effect, aligned, an approved departure, or an unresolved departure
   requiring a decision. Cite the applicable touchstone material. Source:
   DI-nutan; DI-bihud
6. State either that no coordination barrier applies or the complete barrier
   terms: work allowed before the stop, exact stop point, prerequisite,
   acceptable evidence, named confirmer, local verification, and
   failure-or-change behavior. Source: DI-gumiv
7. For non-trivial work that still needs final DF questions, require any neutral
   TE first and then require a substantive Ninik review of the surviving
   alternatives, proposed question context, and proposed final questions before
   the first question is asked. Require exactly one top-level finding for every
   applicable existing mechanism, with this shape:

   ```markdown
   - Mechanism: <mechanism name>
     - Current treatment: <how the plan uses, omits, or conflicts with it>
     - Severity: <required, recommended, or none>
     - Plan location: <exact plan location>
     - Proposed edit: <concrete edit or none>
     - Disposition: <visible disposition>
     - Reason: <Ninik-based reason>
   ```

   Keep the six child bullets separate, contiguous, and in that order. Do not
   wrap the fields into one paragraph or place the finding beneath an outer
   `Mechanisms` bullet. Combine multiple corrections for one mechanism in that
   single finding while preserving each severity, location, edit, and
   disposition in the corresponding child values. An already-correct treatment
   uses proposed edit `none` and disposition `already satisfied`; when none
   applies, require `Mechanism: no applicable existing mechanism` with the same
   six nested bullets. Required corrections block the question sequence.
   Recommended corrections are incorporated or explicitly dispositioned and
   remain visible in the question context. Do not use standalone `Simpler
   existing mechanism` or `Plan changes` fields. Source: DI-zaniv; DI-pataf
8. Require an affected later question to be reviewed again when an earlier DF
   answer materially changes it or its alternatives. After answers and DIs,
   require review of only the decision-driven final-plan changes unless the
   selected design was not reviewed earlier or materially differs. When the
   answers leave the reviewed plan unchanged, record that instead of repeating
   the full review. Work needing no DF uses the ordinary final pre-code review.
   User approval of the reviewed final plan releases implementation.
9. Require the result to preserve the chronology of any neutral TE, pre-question
   review and mechanism-bullet dispositions, DF answers and DIs, affected later
   review, post-DI plan review, user approval, and implementation deviations. It
   must not substitute a new post-code review for missing earlier evidence.
   Source: DI-torut; DI-latag; DI-gafom

## Verify handoff intake

1. Validate `handoff.json` before relying on the prose. Its owning TODO,
   attempt directory, scratch directory, worktree, branch, commit roles,
   session, worker type, and optional thread identity must agree with observed
   state. For version 2 prove `merge_base_commit -> worker_base_commit -> worker
   tip`; for version 1 prove its historical `worker_base_commit -> worker tip`.
   Source: DI-barir
   Stop on a missing field, disagreement, or authority outside the assignment.
2. Compare the recorded guidance fingerprint with the already-injected guidance
   using `cdint-grid-token-efficiency agents verify`. On a match, do not reread
   `AGENTS.md`. On a mismatch, read every complete changed section emitted by
   the command. Fetch an unchanged named section with `agents section` only
   when its exact text is needed. Source: DI-hasod
3. Confirm that the handoff contains the assignment-level `Ninik Review`. When
   final DF questions remain, confirm that any required neutral TE precedes the
   first substantive review. Confirm that every applicable existing mechanism
   has exactly one top-level `Mechanism` bullet whose separate nested bullets are
   `Current treatment`, `Severity`, `Plan location`, `Proposed edit`,
   `Disposition`, and `Reason`, in that order. Reject wrapped, missing, extra,
   renamed, misordered, or duplicate fields. Confirm that an already-correct
   treatment is marked `none` and `already satisfied`, or that the review uses
   `Mechanism: no applicable existing mechanism` with the same nested shape.
   Required corrections block questioning, recommended corrections retain
   explicit dispositions, and materially affected later questions are reviewed
   again.
4. Confirm that the post-DI review covers the decision-driven final-plan changes,
   or the affected full design when the selection was unreviewed or materially
   different, before user approval releases code generation. The eventual result
   preserves the complete chronology and deviations instead of performing a new
   post-code design review.
5. Confirm either the complete barrier terms or an explicit no-barrier statement.
6. Recheck every path, side-effect, decision, test, result, and stop boundary
   before implementation. Do not classify a verified tracked-`.gitignore`
   artifact as a path failure; its ignore status does not relax any independent
   secret, production-data, side-effect, disk-use, privacy, retention, cleanup,
   or live-system rule. Stop when any other required fact cannot be proved.

## Cross a coordination barrier

Do only the pre-barrier work named in the handoff, then stop before the stated
boundary. Ask the named confirmer once for the exact evidence and do not poll,
focus, interrupt, or invent a queue or protocol mechanism. After confirmation,
read the named evidence from the named source and verify it once as specified.
Reject release by any other actor or with missing or unverifiable evidence. If
the worker is known to have crossed the barrier early, record the noncompliance
and stop; later evidence does not cure it. Do not add a semaphore, queue item,
handoff schema field, validator, or wire message to represent the barrier.
Resume the same unfinished attempt only if its scope, approved paths,
assignment start, merge base, prerequisite, and confirmer are unchanged. A
material change or completed
handback requires a fresh numbered attempt. Source: DI-gumiv; DI-rufim

Keep completed-result intake in the separate `handback` skill. Fingerprints and
bounded extracts reduce repeated model context; they never relax decision-first,
review, merge, recovery, privacy, identity, storage, external-write, comment,
test, or result-report gates.

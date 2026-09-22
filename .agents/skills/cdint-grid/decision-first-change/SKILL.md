---
name: decision-first-change
description: Load exact task and decision context and enforce the decision lock before repository changes. Use for implementation, refactoring, behavior changes, naming choices, or path decisions governed by the repo's decision-first protocol.
---
<!--
TBD: Imported verbatim from captured cdint-grid skill decision-first-change for QMR dogfooding.
This procedure may require cdint-grid executables, worker branches, identity rules,
coordination files, or organization decision records that are not configured in a
QMR consumer. Do not treat this import as approval or portable default policy.
-->

# Decision-First Change

1. Identify the governing TODO file and task ID. Run
   `cdint-grid-token-efficiency todo --source PATH --task TASK` to load the exact
   subtree and active Source/Affects/Supersedes DI closure.
2. Fetch a historical decision only when needed with `--decision DI-ID`; use
   `--list-history` to discover historical IDs without loading their bodies.
3. Load named glossary terms or AGENTS sections with the matching deterministic
   selectors.
4. Gather discoverable facts, then collect architecture, behavior,
   implementation, function naming, variable naming, and path decisions exactly
   as the canonical protocol requires.
5. Append the locked DI entries and present the Decision Lock before editing.
   During implementation, stop if a new unapproved decision or path appears.
   A local artifact matched by a tracked repository `.gitignore` has standing
   path approval under DI-sohig: verify the match and tracked ignore source,
   keep the artifact untracked, and do not report it as path noncompliance.
   Independent secret, production-data, external-side-effect, disk-use,
   privacy, retention, cleanup, and live-system rules still apply.
6. Finish with tests, comment and intent-provenance audits, the decision matrix,
   runtime path matrix, exceptions, and compliance result.

Bounded context changes how source is loaded, not what must be decided or
validated. Source: DI-pugid; DI-hasod

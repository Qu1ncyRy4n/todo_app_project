---
name: commit
description: Create and self-review one Git commit that carries an exact plain-English worker promise. Use whenever a cdint-grid worker, including main, starts work, commits implementation or documentation, records later evidence or correction, or publishes a local kept, broken, or inconclusive evaluation.
---
<!--
TBD: Imported verbatim from captured cdint-grid skill commit for QMR dogfooding.
This procedure may require cdint-grid executables, worker branches, identity rules,
coordination files, or organization decision records that are not configured in a
QMR consumer. Do not treat this import as approval or portable default policy.
-->

# Commit A Promise

Use ordinary Git commits as one worker's durable promise history. Apply this
skill symmetrically on main and every peer branch. Source: DI-vomid; DI-pufuk

## Prepare

1. Read the governing TODO or DR and its active DI closure. Confirm that paths,
   behavior, naming, side effects, barriers, and stop conditions are decided.
2. Verify that the current branch is the worker's stable ID and that `git var
   GIT_AUTHOR_IDENT` reports `<branch> <branch@local.invalid>`. Verify that `git
   var GIT_COMMITTER_IDENT` still reports the repository owner's normal
   identity. Stop on a detached head, another worker's branch, an unexpected
   Author, or an unexpected Committer.
3. Inspect the complete status and diff. Stage only individually named intended
   files. Preserve unrelated changes and never use broad staging.
4. Run the checks that apply to the exact tree being promised. State checks not
   run and unfinished scope in the commit body.

## Write One Promise

Use an imperative, capitalized subject and a readable body. End the final Git
trailer block with exactly one nonempty trailer:

```text
Promise: I promise that this commit <states the exact bounded claim>.
```

Keep the actual promise in that one trailer. Put applicable TODO or DR links,
scope, paths, barriers or their absence, stop conditions or their absence,
checks, evidence, limitations, and unfinished work in ordinary body prose.

- A starting promise may be an empty commit. It names the accepted work and
  limits before substantial edits.
- A code or documentation commit promises only what is true of that exact tree.
- A later check or correction names the full earlier promise commit ID and uses
  a successor commit. Never amend or squash away a published promise.
- A local evaluation names the full evaluated promise commit ID and assesses
  its exact scope as `kept`, `broken`, or `inconclusive`. It is that evaluator's
  promise, not a global verdict.
- A merge commit makes one promise about its combined tree. Imported commits
  keep their own promises.

For materially translated committed peer work, use adjacent pairs:

```text
Source-commit: <full Git object ID>
Co-authored-by: <exact Author of that source commit>
```

Every `Source-commit:` must be immediately followed by its matching
`Co-authored-by:`. Complete peer-tip imports use Git parentage instead. Dirty
working trees and session logs are provisional observations: describe them in
the body, credit material authorship in prose when applicable, and own the new
commit and its tests without inventing a source commit.

## Verify

1. Create the commit without rewriting an existing published commit.
2. Read the complete committed message and diff from Git.
3. Confirm the Author matches the branch worker, the Committer is expected, the
   message has exactly one nonempty `Promise:` trailer, source/co-author pairs
   are adjacent and accurate, and the promise matches the committed tree.
4. If the promise or tree is wrong, preserve it and make a correcting successor
   commit. Do not silently reinterpret it.

## Announce Worker Instruction Changes

1. After creating the commit, load `.agents/worker-instructions.manifest` from
   both its first parent and the new commit when present. Expand their union and
   compare it with the committed path changes. This makes additions, edits, and
   removed dependencies equally visible. Source: DI-josov
2. If no committed path is covered, no instruction announcement is needed. If a
   covered path changed, make the promise body describe the instruction-bundle
   change and its known limitations. After the commit exists, put its exact
   commit ID and resulting manifest blob in each announcement; a commit must not
   attempt to cite its own not-yet-known object ID.
3. On `main`, verify and atomically install any shared host executable whose
   changed behavior the new bundle requires, then run
   `cdint-grid-skills-sync` with no target. Do not broadcast routine main
   upgrades through worker inboxes. Report every worker as current,
   synchronized, skipped, or failed; a skipped worker keeps its prior
   instructions for later investigation. Source: DI-kolat
4. On another worker branch, use `putq` to offer the exact candidate only to
   `main` or other named peers that should evaluate it. Do not broadcast a local
   proposal as though it were globally accepted.
5. A notification failure does not rewrite or invalidate the Git promise. Record
   the unresolved delivery and retry only after inspecting existing CAS and
   transport evidence; never send blindly or poll recipients.

The first valid own-author promise on a branch is that worker's transition into
the active peer process. Every later first-parent commit must satisfy this
skill. Source: DI-goziz; DI-bavon; DI-dabuv; DI-dijul

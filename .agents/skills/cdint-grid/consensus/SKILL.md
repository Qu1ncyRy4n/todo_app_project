---
name: consensus
description: Discover, inspect, evaluate, translate, or merge peer Git promises while preserving local acceptance and exact provenance. Use when any cdint-grid worker, including main, considers peer work, combines branches, records accept/decline/defer feedback, rebuilds its local candidate view, or checks for merge ping-pong.
---
<!--
TBD: Imported verbatim from captured cdint-grid skill consensus for QMR dogfooding.
This procedure may require cdint-grid executables, worker branches, identity rules,
coordination files, or organization decision records that are not configured in a
QMR consumer. Do not treat this import as approval or portable default policy.
-->

# Reach Local Consensus

Each worker decides what to trust and what to promise on its own branch. Main
uses the same algorithm and is special only because it writes the official
branch. There is no global candidate verdict or trust score. Source: DI-bisaf

## Discover Finite Peer Evidence

1. Read the governing TODO or DR, current goal and critical path, and applicable
   repository checks.
2. Enumerate current Git worktrees and relevant peer refs once. Do not poll.
3. For each candidate, inspect its exact commit ID, ancestry, complete promise
   messages, diff, linked TODOs or DRs, and named evidence.
4. Peer working trees and available session logs may supply useful provisional
   observations. They are not stable promises. Record what was observed and
   take responsibility for any resulting implementation.
5. Build or refresh one disposable local view per peer at
   `/tmp/cdint-grid/consensus/<worker-id>/peers/<peer-id>.md`. Name the exact
   repository/ref frontier, evidence examined, derivation rules, contextual
   assessment, limitations, and Git sources from which the file can be rebuilt.
6. Inspect the complete candidate for every action that the producer expects a
   different worker to perform. Verify that each distinct completion gate has an
   unchecked TODO naming the intended worker, exact action, completion gate, and
   source evidence. A decision request must cite its authoritative DR and have a
   TODO pointing to it. Source: DI-lobun

## Route Worker Instruction Changes

1. Load `.agents/worker-instructions.manifest` from the receiver and candidate
   commits when either exists. Detect every candidate change covered by either
   manifest, including deletion and manifest edits.
2. Do not pass peer instruction proposals to `upgrade`.
   `cdint-grid-skills-sync` copies only the write set already committed on local
   `main`; it does not evaluate a peer candidate. Source: DI-kolat
3. A non-main worker may inspect and evaluate the proposal, but it records the
   exact candidate for main rather than creating a divergent automatic copy of
   main-owned instructions. Main may decline it, translate it, or integrate it
   through ordinary consensus and a main promise commit.
4. For a candidate containing both instruction-bundle and product changes,
   evaluate the two scopes separately. Do not merge the complete peer tip until
   the receiver has dispositioned both scopes.
5. After main commits an accepted instruction change, the `commit` procedure
   verifies required shared executables and runs all-worker deterministic
   synchronization. A skipped worker retains its earlier instructions for
   investigation; the skip is not a global verdict on the peer proposal.
6. DI-javur's upgrade-then-drain sequence remains historical evidence for the
   former model-mediated upgrade flow. Current `resume` runs deterministic
   synchronization and then performs its one finite drain; `consensus` does not
   add a second drain.

## Choose A Voluntary Action

The receiving worker may do nothing, disagree, ask for a decision in a DR,
create repair work in a TODO, translate or reimplement an idea, or import the
complete peer tip. Reading peer evidence never compels acceptance.

- Translation or reimplementation uses the `commit` skill and exact
  `Source-commit`/`Co-authored-by` pairs when committed peer work materially
  contributed.
- A complete import records the selected peer commit as a real Git parent only
  after the receiver has examined and dispositioned that whole tip.
- A material accept, decline, or defer decision is an evaluator-authored promise
  commit. Use `kept`, `broken`, or `inconclusive` only against the exact scope of
  the named promise.
- If the producer omitted a required expected-action TODO, create a safety-net
  TODO on the receiving worker's branch before committing an accept, decline, or
  defer promise. Record the producer omission as noncompliance and do not claim
  that the producer satisfied the origin-worker rule. Update or link an existing
  TODO instead of duplicating it. The safety net preserves the work; it does not
  compel the intended worker or rewrite the producer's history. Source: DI-lobun

## Integrate A Complete Peer Tip

1. Commit or checkpoint permissible local work with the `commit` skill.
2. Name the receiver's current commit and the exact peer commit to evaluate.
3. Determine checks from the governing TODO or DR, established repository
   gates, and the peer's evidence. Do not invent an unrelated gate.
4. Before merging, warn when the peer is already contained, the proposed merge
   changes no tree, only reciprocal merge commits are new, or the same branches
   repeatedly exchange merges without substantive work. A warning does not
   prohibit a merge carrying code, tests, decisions, or conflict resolution.
5. Run `git merge --no-ff --no-commit <exact-peer-commit>` on the receiving
   branch. Resolve conflicts there. Run the selected checks against the exact
   combined tree.
6. If checks fail, repair the receiving branch and rerun them. Stop only for an
   unresolved decision, unapproved path or side effect, changed governing scope,
   or another explicit stop condition.
7. Before committing, confirm the receiver parent and selected peer object IDs
   still identify the tested inputs. If a peer ref advanced, either continue
   with the already selected immutable commit and state that the newer tip was
   not evaluated, or restart with the newer tip.
8. Commit the exact tested tree with the `commit` skill and one promise about
   the combined result. Run any required after-merge checks and correct failures
   with successor commits.

An integration specialist follows these same steps on its own branch. Main may
publish that candidate, another candidate, or none. The existing merge queue is
legacy compatibility or optional mechanical checking; FIFO position and queue
admission do not create acceptance. Source: DI-bavon; DI-dabuv; DI-vomid

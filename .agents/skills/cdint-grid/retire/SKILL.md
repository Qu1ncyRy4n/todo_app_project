---
name: retire
description: Verify and retire completed ordinary Codex workers without losing branch, promise, or legacy result evidence. Use when the user asks to retire workers, close completed worker sessions, or remove their linked worktrees. Do not use for product, data, service, or employee retirement.
---
<!--
TBD: Imported verbatim from captured cdint-grid skill retire for QMR dogfooding.
This procedure may require cdint-grid executables, worker branches, identity rules,
coordination files, or organization decision records that are not configured in a
QMR consumer. Do not treat this import as approval or portable default policy.
-->

# Retire Worker

Retire ordinary Codex workers only after every durable promise and obligation
has a local disposition and the user gives exact approval. Never infer
eligibility from an inventory hint, merged commit, clean worktree, promise count,
or legacy result summary alone. Source: DI-jupiz; DI-tobih; DI-sohah; DI-pufuk

## Inventory

1. Run only from the main checkout on branch `main`.
2. Run exactly one `/home/stevegt/bin/cdint-grid-workers check`. Verify the
   installed executable against main's recorded source commit and SHA-256
   evidence first. Retain its snapshot path and read the generated inventory.
   Do not compile the tool or poll. Source: DI-dogas
3. Iterate over every discovered worker in the inventory's deterministic order.
   Report standing workers, but do not apply ordinary-worker retirement to the
   dobab or overall-review standing workers.
4. Treat inventory retirement-candidate reasons as leads to verify, never as
   retirement authority. An unrelated worker's warning does not block a target
   whose mandatory evidence is available; a target-specific error or unavailable
   mandatory evidence does.

## Verify Each Ordinary Worker

For each ordinary worker, perform all checks below. When one fails, state the
exact reason, leave that worker intact, and continue to the next worker.

1. Identify the exact stable branch, worktree, tmux session, current branch tip,
   and `promise_commits` inventory evidence. For an old-style worker, also name
   its current attempt directory, assignment-start commit, merge-base commit,
   and submitted tip. Stop that worker's retirement evaluation on ambiguity.
2. When the branch has transitioned, read its complete first-parent promise
   history from the transition commit through the current tip. Apply `consensus`
   to every implementation, evidence, correction, and evaluation promise and
   every linked TODO or DR. For a legacy worker, instead apply `handback` to the
   complete packet and all referenced evidence.
3. Map every promise or legacy result, recommendation, new file, commit, and
   branch change to a durable local disposition: merged or translated work, a
   `kept`, `broken`, or `inconclusive` evaluation, an owned TODO or DR, or an
   explicit decline or defer promise with rationale. Missing or summary-only
   dispositions block retirement.
4. Main must consider the exact worker tip through the same `consensus` process
   as any other peer. The tip need not be an ancestor of main when main has
   durably translated it, evaluated it, or deferred it. A branch may be
   explicitly abandoned only with the repo owner's approval recorded in a
   promise and applicable TODO or DR. Preserve the branch either way.
5. Verify required accounting and TODO or calendar reconciliation are recorded,
   no promise, assignment, merge, or legacy queue entry remains active, and the
   worktree has no merge, rebase, cherry-pick, revert, bisect, staged change,
   unstaged change, or untracked file.
6. Read the current-pane capture retained by `workers check`. Require it to agree
   that the worker is complete and idle. Working output, a pending question,
   requested approval, unresolved blocker, ambiguous status, or an unavailable
   pane blocks retirement.

## Approve And Retire

1. For each eligible worker, show the exact tmux session, worktree, branch,
   branch tip, consensus disposition or approved-abandonment evidence, and evidence that
   will remain. Ask for one worker-specific approval covering both session
   termination and subsequent non-forced worktree removal. Do not batch several
   workers into one approval.
2. If approval is declined, leave the worker unchanged and continue the loop.
3. After approval, terminate only the named tmux session. Inspect the command
   status and verify independently that the exact session is absent. If
   termination fails or absence cannot be proved, do not remove the worktree;
   report the failure and continue.
4. Reverify the exact branch tip, clean worktree, absent Git operation, retained
   evidence, and absent tmux session. Remove the exact linked worktree with
   ordinary non-forced `git worktree remove`. Never use `--force`.
5. Inspect the removal status. Verify the worktree is absent from
   `git worktree list`, while its branch, promise commits, legacy packets,
   reviews, queue records, and run evidence remain. A removal failure leaves the worktree in place
   for diagnosis; do not retry destructively.
6. Continue until every discovered ordinary worker has been evaluated.

## Final Report

Run one final `/home/stevegt/bin/cdint-grid-workers check` to refresh private
inventory state without rebuilding the tool. Report four lists: retired,
skipped as ineligible, approval declined, and
blocked by a lifecycle failure. For every non-retired worker, give the exact
next action. Then add a distinct `Idle But Ineligible` list containing every
discovered worker whose retained current-pane capture is classified as idle but
which failed any retirement requirement. For each, state the exact failed
requirement and next action; do not replace this complete list with examples or
a count. Include idle standing workers and state that their standing role makes
them ineligible for ordinary-worker retirement. Never delete worker branches,
commits, retained attempt records, reviews, queue evidence, or run evidence.
Source: DI-jupiz; DI-bogat

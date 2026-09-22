---
name: resume
description: Return an idle cdint-grid worker to useful work by first ensuring current instructions, then draining one finite inbox snapshot and working prioritized unblocked items. Use when the user says resume, when newtree starts or reuses a worker, or when mainloop resumes an idle peer.
---
<!--
TBD: Imported verbatim from captured cdint-grid skill resume for QMR dogfooding.
This procedure may require cdint-grid executables, worker branches, identity rules,
coordination files, or organization decision records that are not configured in a
QMR consumer. Do not treat this import as approval or portable default policy.
-->

# Resume An Idle Worker

Use this procedure for one bounded work cycle. It joins existing mechanisms; it
does not grant task, path, side-effect, decision, or barrier authority. Source:
DI-fivat; DI-kumof; DI-kolat

## Establish The Cycle

1. Invoke `upgrade` before inbox work. In a non-main worker this runs
   `cdint-grid-skills-sync <stable-worker-branch>`; in main it considers every
   linked non-main worker. A stale worker may read the current `upgrade` and
   `resume` skills from local `main` only to bootstrap this ordering. Do not use
   `getq`, `drain-inbox`, an inbox message, or `consensus` as a substitute.
2. Identify the stable worker ID, current branch and worktree, governing TODO or
   DR, accepted scope, and any barrier that limits the work. If an atomic action
   is already running, finish it or stop it truthfully before beginning this
   cycle.
3. If synchronization skips this worker or fails, record the exact reason,
   notify `main` once through the existing work-state path or `putq`, and stop.
   Do not drain the inbox or begin another task under stale or partially copied
   instructions. Another worker being skipped does not block a worker that the
   same all-worker run synchronized successfully.
4. After successful synchronization or an exact current result, confirm the locally
   accepted `AGENTS.md` guidance under the deterministic context-loading rules;
   do not reread an unchanged complete file. Read the complete local
   `drain-inbox`, `putq`, `consensus`, and `commit` skills before using them.
   Local accepted files, not the source worker's working tree, govern the rest
   of this cycle.

## Drain And Prioritize

1. Invoke `drain-inbox` exactly once. Complete its frozen-snapshot mapping and
   prioritization before doing any mapped work.
2. Build the eligible work set from the explicit governing task for this cycle
   and the mapped inbox work. Keep only items inside the worker's accepted scope
   and current barriers. Repository priorities, hard failures, dependencies,
   and the explicit task determine order; inbox arrival alone does not.
3. Select the highest-priority unblocked item. If an item is blocked, record the
   exact blocker under its TODO or DR, notify `main` once, and continue to the
   next unblocked item. Never cross the blocker merely to keep the worker busy.
4. If no unblocked item remains, report that result and stop. Do not poll,
   repeat `drain-inbox`, or chase messages that arrived after the snapshot.

## Work And Report

1. Work the selected item under its accepted decisions, paths, side effects,
   checks, and stop conditions. Obtain any missing decisions and path approvals.
   Before product edits, run the required Ninik review and use `commit` to create
   the starting promise. Follow WALTAL, and use `commit` for each later durable
   Git promise.
2. Invoke `consensus` when this selected item needs peer code, peer promises,
   provisional peer observations, peer integration, or when the worker's branch
   is behind `main`. The drain completed earlier in this cycle satisfies
   consensus's post-upgrade drain requirement; tell consensus to reuse that
   frozen snapshot and not drain again. Do not use `consensus` as a substitute
   for `upgrade` or as a global preflight. Source: DI-javur
3. On completion, update the governing TODO or DR and notify `main` once with
   the worker ID, governing item, exact promise or result, checks, limitations,
   and remaining work. Reuse an already-required work-state notification when
   it carries those facts; otherwise use `putq`. When the current worker is
   `main`, record and report the same facts locally instead of sending a
   self-message.
4. Continue with the next highest-priority unblocked item from the same work
   set. Apply the same blocker and completion rules. Stop when no unblocked item
   remains or a governing stop condition requires it.

One invocation synchronizes the instruction write set first, then processes one
finite inbox snapshot and the resulting bounded work set. A later invocation
runs the deterministic check again before beginning another cycle.

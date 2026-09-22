---
name: newtree
description: Create or reuse a stable branch-based cdint-grid peer worker without creating an attempt packet. Use when the user says newtree or asks to start a fresh worker for a governing TODO or DR while keeping main available for coordination.
---
<!--
TBD: Imported verbatim from captured cdint-grid skill newtree for QMR dogfooding.
This procedure may require cdint-grid executables, worker branches, identity rules,
coordination files, or organization decision records that are not configured in a
QMR consumer. Do not treat this import as approval or portable default policy.
-->

# Start A Stable Peer Worker

Perform only enough work in the current session to establish a non-conflicting
worker and start it. The worker performs task-specific planning and begins by
publishing its own starting promise. Source: DI-pufuk

## Establish The Worker

1. Identify the governing TODO or DR and the user's approved stable worker ID.
   The worker ID, branch name, and new worktree basename are the same slug. Do
   not create an attempt ID or roll the name for a new task or Codex process.
   Reject a worker ID with an `-aNN` attempt suffix. Source: DI-bibur
2. Inspect all worktrees, branches, tmux sessions, active Git operations, and
   dirty state. Reuse a related stable worker when it can continue safely;
   otherwise create the approved branch and linked worktree. Never edit or push
   another worker's branch.
3. Enable `extensions.worktreeConfig=true` in the repository. In the selected
   worktree, set `author.name=<worker-id>` and
   `author.email=<worker-id@local.invalid>` with `git config --worktree`. Do not
   replace `user.name` or `user.email`; those normal repository identity values
   remain the Committer.
4. At a legacy worker's safe restart, reconcile its suffix-free branch,
   worktree basename, Author, inbox identity, tmux identity, and inventory
   identity before later peer work. Preserve historical names and local-CAS
   evidence; do not rewrite them. Source: DI-bibur
5. Resume an idle worker by invoking the current `resume` skill directly.
   `resume` uses `upgrade` as the compatibility command for deterministic
   synchronization from local `main` before it drains the inbox or begins work,
   then rereads the resulting local instructions. Do not put routine instruction
   synchronization in an inbox message or ask a stale worker to run `getq`
   first. Source: DI-fivat; DI-kumof; DI-kolat

## Start Codex

1. Create bounded mode-`0700` runtime state beneath
   `/tmp/cdint-grid/workers/<worker-id>/`; files use mode `0600`. A generated
   `start.sh` and prompt files are disposable local launch machinery, not task
   authority or durable results.
2. The launcher starts or reuses the worktree-derived tmux session with
   `CODEX_TMUX_CODEX_COMMAND='tools/attempt-env -- codex -c model_reasoning_effort=high -c
   plan_mode_reasoning_effort=xhigh' codex-tmux --create-only`, waits for the
   initialized TUI, sends `/plan` once through `tools/codex-send-prompt`, and
   verifies visible Plan mode. The wrapper gives the worker and its child Go
   commands the shared build and module caches; it does not create a
   worker-specific Go cache. Source: DI-dogas
3. When the existing TUI already knows the current `resume` skill, invoke that
   skill directly. For a stale TUI, send one bootstrap instruction naming the
   exact main commit and
   `git show <commit>:.agents/skills/resume/SKILL.md`, then tell the worker to
   run that exact procedure. In both cases, `resume` performs deterministic
   instruction synchronization before work resumption in one turn. Source:
   DI-kumof; DI-kolat
4. Any separate short task instruction names the governing TODO or DR and
   accepted repository, path, and side-effect scope. The task's existing decisions and
   stop conditions still govern; `resume` grants no authority and preserves the
   required decision, path-approval, Ninik-review, and starting-promise gates.
   For a long instruction, place the text in a mode-`0600` file beneath the
   worker runtime root and send only its absolute path before invoking
   `resume`.
   Keep authority separate from execution capability: never direct a worker to
   use a Unix writability probe as authority or as proof of Codex sandbox or
   Git-index capability. Source: DI-furab; DI-fivat
5. Capture the complete visible pane before and immediately after every send.
   Verify that the instruction was submitted and work began. Never resend
   blindly and never poll in a sleep loop.
6. Open one visible terminal with `codex-tmux --attach-in-new-terminal` after
   the session exists. Verify that the expected session is attached.

No `handoff.json`, `handoff.md`, `results.md`, merge declaration, or numbered
attempt directory is created for new-style work. The governing TODO or DR is
the durable request; Git promise commits are the worker's durable response.

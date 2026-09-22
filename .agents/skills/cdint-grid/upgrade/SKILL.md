---
name: upgrade
description: Deterministically synchronize main-owned cdint-grid worker instructions into linked worktrees. Use whenever the user says exactly `upgrade`, resume requires current instructions, or main has committed a manifest-covered instruction change.
---
<!--
TBD: Imported verbatim from captured cdint-grid skill upgrade for QMR dogfooding.
This procedure may require cdint-grid executables, worker branches, identity rules,
coordination files, or organization decision records that are not configured in a
QMR consumer. Do not treat this import as approval or portable default policy.
-->

# Synchronize Worker Instructions

`upgrade` is the compatibility command for deterministic main-to-worker
instruction synchronization. It does not ask a model to compare or rewrite
skills. Product code, TODOs, design notes, live calendar state, and peer design
decisions remain outside this mechanism and use `consensus` when peer evaluation
is needed. Source: DI-kolat

## Run The Synchronizer

1. Confirm the current stable branch and linked worktree. Do not finish an
   already-running atomic write by starting synchronization concurrently.
2. From a non-main worker, run
   `cdint-grid-skills-sync <stable-worker-branch>`. From `main`, run
   `cdint-grid-skills-sync` with no target so every linked non-main worktree is
   considered. Do not fetch or substitute a remote branch; the unique local
   `main` worktree is the source.
3. Exit `0` means every selected worker was already current or received one
   verified worker-authored synchronization commit. Exit `2` means at least one
   worker was skipped safely; read every named reason and investigate rather
   than forcing or deleting worker state. Exit `1` means the run stopped after
   an operational failure that may require repair before another write.
4. After success in a worker, reread the resulting local `AGENTS.md`, manifest,
   and changed skills through deterministic context loading before beginning
   another task. A TUI restart is not required.

## Safety Boundaries

- Main must have a clean committed manifest write set. Unrelated dirty paths on
  main or a worker do not block synchronization and must remain unchanged.
- The manifest is main's coordination write set. It is not product authority,
  task scope, a design verdict, or permission to change paths outside that set.
- Identical bytes are current regardless of timestamp. Newer differing worker
  bytes, equal-time differing bytes, worker-only covered files, dirty covered
  paths, or identity mismatches require human investigation and are never
  overwritten automatically.
- `rsync` runs without `--delete`. The synchronizer neither builds nor installs
  tools nor removes ignored local executables.
- A synchronization commit promises only the mechanical instruction copy from
  one exact main commit. It does not accept another peer's product or design
  work. Evaluate those candidates with `consensus`.
- `--dry-run` or `-n` may be used for a read-only comparison. It creates no
  lock, report, commit, or filesystem change.

Historical handoff packets that explicitly require the former model-mediated
upgrade procedure remain historical evidence. New and resumed peer workers use
this deterministic procedure.

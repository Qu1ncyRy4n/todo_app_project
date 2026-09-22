---
name: git-change-finalization
description: Review, stage, and commit a focused change without taking external Git actions unexpectedly. Use when preparing a commit or assessing a working-tree change.
---
# Finalize A Focused Git Change

## Inspect Before Staging

Inspect repository status and the focused diff. Preserve unrelated uncommitted
work. Confirm that the changed files implement one coherent requested behavior
and that validation evidence matches that behavior.

## Stage Explicitly

Stage only the files and hunks intended for the requested change. Do not stage,
unstage, revert, rewrite, or clean up unrelated work without explicit
instruction.

## Select The Commit Mode

Use the task's stated mode. Default to **commit only when asked**: prepare and
validate the change, then wait for a commit request. In **goal -> atomic
commits** mode, commit each independently valid milestone toward the explicitly
authorized goal. Do not use either mode to commit unrelated work.

When committing, use a short, imperative, capitalized subject. Summarize
non-trivial changes by file in the body when useful. Do not add agent,
model-provider, or company attribution.

Do not force-push, open a pull request, publish, or otherwise make an external
Git change unless the developer explicitly asks.

## Verify

Inspect the final staged diff or commit and report the validation run, remaining
risk, selected commit mode, and any intentionally unstaged changes.

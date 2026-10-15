---
name: todo-ledger
description: Read and update the developer's global todo ledger through the `todo` CLI without cluttering it. Use when a task mentions todos, the ledger, sprints, item IDs such as `kohil-dojul`, or asks what to work on next.
---
# Work With The Todo Ledger

The `todo` CLI is the complete surface for one global, event-sourced ledger.
Every write is an immutable event; nothing is truly deleted, but clutter costs
the developer attention. Read freely; write deliberately.

## Read First

```sh
todo ls --global --tree          # everything open, with structure
todo ls --global --tree --all    # include done and canceled
todo show <id> [--history]       # notes, citations, relations
todo next --global --explain     # what the ranking would pick, and why
todo ready --global              # open, undeferred, unblocked
todo related-to <id>             # inverse view of typed relations
todo tag ls                      # namespace paths in use
```

IDs are proquints (`kohil-dojul`); a unique trailing fragment also works.
`todo ls` has no tag filter; pipe through `grep` for a tag path. The working
directory may not be attached to a project, so prefer `--global` and identify
work by tag (for example `tag/work/apps/todo-app`) and parent.

## Write Only What Was Asked

- Add an item only when the developer asks, or when they approve a list you
  proposed. Never capture speculative follow-ups on your own.
- Place items under the right parent (`--parent <id>`) and reuse existing tag
  paths from `todo tag ls`; do not invent new top-level namespaces.
- Prefer `todo note <id> "…"` to record progress, findings, or decisions over
  editing titles. Begin agent notes with `agent-assistant:`.
- Mark state truthfully: `todo start` when work begins, `todo done` only after
  the developer has confirmed anything they must verify (for example a GUI
  change seen in a running window).
- For several writes, list them first, then run them as one script and show
  the resulting `todo ls --tree` slice.

## Undo Without Destroying

- An item created in error: `todo void <id>` (reversible with `todo reopen`).
- Work deliberately dropped: `todo cancel <id>`.
- Never run `todo rm --force`.
- A wrong parent, tag, or dependency: fix it with `todo parent`,
  `todo tag remove`, or `todo undepends`; the history keeps both.

## Approval Boundaries

- `todo sync` pushes to a Git remote. Do not run it unless asked.
- `todo backup`, `todo import`, and `todo export` touch the filesystem; run
  `todo import … --dry-run` first and show the result.
- The graph editor (`todo-gui`) may be open; the CLI can write while it runs,
  and the editor refreshes within a second.

## Verify

After writing, show the affected slice (`todo ls --global --tree | sed -n …`
or `todo show <id>`) and report exactly which items changed.

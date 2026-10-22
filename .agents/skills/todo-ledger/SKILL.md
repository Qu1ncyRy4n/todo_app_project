---
name: todo-ledger
description: Read and update the developer's global todo ledger through the `todo` CLI without cluttering it, signing every write as an agent. Use when a task mentions todos, the ledger, sprints, item IDs such as `kohil-dojul`, or asks what to work on next.
---
# Work With The Todo Ledger

The `todo` CLI is the complete surface for one global, event-sourced ledger.
Every write is an immutable event; nothing is truly deleted, but clutter costs
the developer attention. Read freely; write deliberately.

## Check The Signature First

Every event an agent writes carries an `agent` field (protocol v15), so the
developer can filter agent work out. Before the first write in a session, run:

```sh
todo whoami    # device: <id>   agent: <name>   me: <name>
```

- `agent:` must name you. The CLI signs as `agent` when it sees `CLAUDECODE`,
  `GEMINI_CLI`, or `CODEX_SANDBOX` (configurable as `agent_env`).
- Prefer `TODO_AGENT=agent-assistant todo …` so the signature matches the
  `agent-assistant` actor and note prefix.
- If `agent:` is empty, stop and say so rather than writing as the developer.
- `TODO_AGENT=` (empty) writes as the developer. Use it only when they ask
  for a write in their own name.

## Read First

```sh
todo ls --global --tree          # everything open, with structure
todo ls --global --tree --all    # include done and canceled
todo ls --global --agent only    # only agent-created items (also hide, show)
todo show <id> [--history]       # notes, citations, relations; history shows signers
todo next --global --explain     # what the ranking would pick, and why
todo ready --global              # open, undeferred, unblocked
todo related-to <id>             # inverse view of typed relations
todo tag ls                      # namespace paths in use
todo actor [name]                # people and agents, their fields and roles
```

IDs are proquints (`kohil-dojul`); a unique trailing fragment also works.
`todo ls` has no tag filter; pipe through `grep` for a tag path. The working
directory may not be attached to a project, so prefer `--global` and identify
work by tag (for example `tag/work/apps/todo-app`) and parent.

Agent-created items list as `[agent]` or `[agent:<name>]`. `--agent` on `ls`,
`ready`, and `next` defaults to the developer's `agent_items` setting, so a
listing may already hide agent work; pass `--agent show` to see everything.
An item counts as agent-created when an agent signed its creation, or when
its author is an actor with `kind = "agent"`; that second rule covers items
made before signatures, which carry `by:agent-assistant`.

## Write Only What Was Asked

- Add an item only when the developer asks, or when they approve a list you
  proposed. Never capture speculative follow-ups on your own.
- Place items under the right parent (`--parent <id>`) and reuse existing tag
  paths from `todo tag ls`; do not invent new top-level namespaces.
- `todo add` takes typed words beside the title: `tag:a/b`, `p1`, `due:…`,
  `est:30m`, `dep:<id>`, `to:<actor>`, `by:<actor>`. Quote a phrase to keep it
  as title. `todo set <id> <words>` applies the same words later.
- Leave authorship to the signature. Do not add `by:<developer>` to your own
  items; add `by:` or `to:` only when the developer names someone.
- Do not create actors or change their fields (`todo set <name> key=value`,
  `todo manifest`) unless asked; those records describe people.
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

- `todo sync` pushes to a Git remote. Do not run it unless asked. Machines on
  a CLI older than protocol v15 reject signed events, so say so if sync is
  requested and you cannot confirm every machine is updated.
- `todo backup`, `todo import`, and `todo export` touch the filesystem; run
  `todo import … --dry-run` first and show the result.
- The graph editor (`todo-gui`) may be open; the CLI can write while it runs,
  and the editor refreshes within a second. The editor always writes as the
  developer, so do not make agent edits through it.

## Verify

After writing, show the affected slice (`todo ls --global --tree | sed -n …`
or `todo show <id>`) and report exactly which items changed. Confirm new items
show `[agent]`.

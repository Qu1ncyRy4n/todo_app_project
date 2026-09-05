# CLI UX Audit: 2026-09-04

## Scope

This audit exercised the built `todo` binary as a skeptical operator, reviewed
all CLI end-to-end tests, and compared the interaction model with Mogent in
`../Agents`. It covered initialization, help and typo recovery, project
attachment, capture, hierarchy, focus, history, import/export, backup restore,
and concurrent invocations using isolated configuration and data directories.

## Findings

### High: transient database locks escaped through the CLI

Forty simultaneous read-only commands caused most processes to fail while
opening Turso with `File is locked by another process`. SQL busy handling could
not help because the failure occurred before a connection existed. Bounded
retry for lock-specific open failures now keeps short-lived human and agent
commands from exposing this storage race. An end-to-end test starts sixteen
readers against one isolated ledger.

This makes concurrent bursts usable, but it does not establish multi-writer or
distributed concurrency semantics. Those remain stage-three decisions under
DI-0007.

### Medium: operators cannot see the active scope before writing

Directory inference is convenient but mostly invisible. `todo project ls`
shows mappings, while `todo ls` and `todo add` do not say whether they selected
a project, the default project, or the global inbox. The first recommended UX
feature is a compact `todo status` command reporting the database path, current
directory, selected scope, and why that scope won. This should precede more
display customization.

### Medium: project configuration lacks correction commands

Projects can be attached and made default, but there is no detach operation and
no way to clear the default through the CLI. Manual configuration editing is
the only recovery from stale mappings. Add explicit `project detach` and
`project default --clear` behaviors after recording their configuration-event
and error semantics.

### Medium: backup creation is not transactional as a directory

The destination is protected from overwrite, but a write failure after
directory creation can leave a partial directory that blocks a retry at the
same path. A later hardening slice should write to a sibling temporary
directory, sync the required files, and rename it into place. The generated
recovery instructions correctly disclose that Markdown restoration loses IDs
and richer facts.

### Low: discovery is adequate but not yet polished

Clap provides useful hierarchical help and typo suggestions. Command and flag
names are generally direct, errors include recovery for initialization and date
input, and destructive replacement is avoided. Shell completion is absent,
successful mutations do not consistently suggest the next useful command, and
there is no stable machine-readable command output for agents.

### Low: the command surface will become difficult to navigate

The flat lifecycle commands are fast for normal use, but top-level help already
contains twenty commands. Future additions should favor coherent groups and
avoid adding display flags independently to every command.

## What Mogent Demonstrates

Mogent's strongest lesson is its development process, not any one rendering
choice. Its dogfood sessions state setup, command, expected result, files that
may change, and a structured report. That turns subjective UX impressions into
repeatable product evidence.

Its centralized command registry also feeds dispatch, help, completion, and
typo suggestions. Clap already supplies most of that consistency for `todo`, so
replacing Clap would add complexity without benefit. The reusable ideas are
staged human exercises, preview-before-write for broad changes, contextual
recovery hints, shell completion, and status output that explains current
state.

Mogent's extensive tree display controls solve a source-navigation problem that
the todo ledger does not yet have. Character sets, alignment modes, terminal
fitting, and a TUI should wait for observed todo-specific pressure.

## Recommended Order

1. Keep the concurrent-open regression test and bounded lock retry.
2. Add a repeatable dogfood script or guide with expected outputs and a report template.
3. Design `todo status` as the single scope and storage explanation command.
4. Add project detach and default-clear recovery paths.
5. Make backup publication atomic at the directory level.
6. Add generated shell completion when regular dogfooding demonstrates demand.
7. Define stable JSON output before relying on the CLI as an agent protocol.

The CLI is usable for single-user dogfooding after the lock fix. It is not yet
polished enough to call self-explanatory for a new user, and it should not yet
be treated as a reliable multi-agent protocol surface.

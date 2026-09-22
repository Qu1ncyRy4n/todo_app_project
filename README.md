# Todo ledger (working name)

A global, local-first todo and delegation ledger. The first release is focused
on managing real work well enough to dogfood one project while retaining a path
toward PromiseGrid messages and multi-agent collaboration.

The product name is intentionally unresolved. `todo` is the provisional CLI
name.

## Why another todo app

Other tools limit extensibility. This one targets:

- Rust
- hierarchical organization
- automatic prioritization / selection
- better visualization through a time-graph

## Current model

```text
one global ledger
├── global inbox
├── projects (logical scopes)
├── stable hierarchical namespaces
└── immutable events
    ├── item changes
    ├── notes
    ├── citations
    ├── structural relationships
    └── focus facts
        ├── priority and dates
        └── blocking dependencies
```

Projects do not own separate databases. A global configuration maps working
directories to project scopes, and an explicit `--project` flag overrides the
mapping.

## Development flow

```text
TE / DR / DI -> domain API -> tests -> Turso store -> tests -> CLI -> tests
             -> dogfood handoff -> feedback -> repeat
```

Enter the development shell with `nix develop`. The eventual installation
command is `cargo install --path crates/todo-cli`; `todo init` initializes the
global ledger and configuration rather than installing the executable.

## Development shortcuts

`./dev` runs the matching Cargo command inside the pinned Nix shell, so it also
works when Rust is not globally installed:

```sh
./dev build
./dev test
./dev check
./dev run -- sync --help
./dev install
```

The enforced checks, lint policy, and intentionally deferred testing and
error-safety work are documented in [development tooling and quality policy](docs/development.md).

## Dogfooding quick start

```text
cargo install --path crates/todo-cli
todo init
todo project attach todo --path /path/to/this/project

todo add "Dogfood the first workflow" --tag goal/todo/v0_1
todo ls --tree
todo ongoing <id>
todo priority <id> high
todo due <id> 2026-09-04
todo next --explain
todo note <id> "What happened while doing it"
todo cite <id> docs/architecture.md --label design
todo done <id>
todo show <id> --history
todo backup /path/to/new/backup-directory
todo sync init --repo /path/to/todo-ledger
todo sync
```

`todo backup` writes exact event bytes, a full JSON view, and Markdown files
that can be imported through the CLI into a clean ledger. The destination must
not already exist, so an older recovery point is never overwritten. See
[DI-0009](DI/DI-0009-tiered-dogfood-backup.md) for the recovery guarantees and
known fidelity limits.

Item references may be full UUIDs or a unique trailing fragment printed by the
CLI. Outside an attached directory, items enter the global ledger without a
project. Use `--project todo` to override directory inference, and `todo ls
--global` to see all open items across project scopes.

Hierarchical namespace examples include `project/todo`, `goal/todo/v0_1`,
`role/developer`, and `topic/storage/turso`. Their paths can be renamed with
`todo tag mv`; assignments retain stable internal IDs and the old path remains
an alias.

See [the versioned tutorial](docs/tutorial.md) for complete capture, focus, and
backup workflows.

For a practical mapping from weekly Obsidian task notes into global and
project-scoped work, see [the weekly-notes tutorial](docs/weekly-notes-tutorial.md).

## Current direction

Development proceeds from one working local global ledger, to interaction with
one-to-three repositories, and then to personal/shared spaces and multiple
users. Exact v5 events can now be synchronized through an explicit Git
checkout; each device keeps its database as a rebuildable local view. See
[DI-0007](DI/DI-0007-staged-global-to-shared-rollout.md).

The current PromiseGrid and `todo-report` comparison is recorded in
[the integration review](docs/integration-review-2026-09-04.md). Direct Grid
reuse and a possible Go migration remain open under
[DR-0002](DR/DR-0002-promisegrid-runtime-and-language.md). Editable Markdown,
including a possible `TODO/TODO.md` layout and readable stable IDs, is deferred
under [DR-0003](DR/DR-0003-editable-markdown-identity.md).

The latest hands-on command review and prioritized usability gaps are recorded
in [the CLI UX audit](docs/cli-ux-audit-2026-09-04.md).

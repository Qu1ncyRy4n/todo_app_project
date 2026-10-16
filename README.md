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

## Control surfaces

Two clients read the same ledger, and neither owns data the other cannot see:

- `todo`, the CLI, which is the complete surface for capture, focus, import,
  export, backup, and synchronization;
- `todo-gui`, a native graph editor for arranging, grouping, searching, and
  inspecting the same items. Run it with `./dev gui`.

The editor is presentation over the same events. Its appearance is configured in
`theme.toml` and its node placement in `gui-layout.json`, both beside
`config.toml`; deleting either loses arrangement, never work. See
[the graph editor handoff](docs/gui-graph-pickup.md) and the
[graph design language](docs/gui-graph-design-language.md).

## Development flow

```text
TE / DR / DI -> domain API -> tests -> Turso store -> tests -> CLI -> tests
             -> dogfood handoff -> feedback -> repeat
```

Enter the development shell with `nix develop`. Install the CLI through
`./dev install`; `todo init` initializes the global ledger and configuration
rather than installing the executable.

Install the graph editor separately with `./dev install-gui`, then launch it
with `todo-gui`. Both executables are installed in Cargo's bin directory.

## Development shortcuts

`./dev` runs the matching Cargo command inside the pinned Nix shell, so it also
works when Rust is not globally installed:

```sh
./dev build
./dev test
./dev check
./dev run -- sync --help
./dev install
./dev install-gui
```

The enforced checks, lint policy, and intentionally deferred testing and
error-safety work are documented in [development tooling and quality policy](docs/development.md).

To pick the project up cold, including what is built, what is deliberately
absent, and what is unverified, start from [the project handoff](docs/handoff.md).

## Dogfooding quick start

```sh
./dev install
export PATH="$HOME/.cargo/bin:$PATH"
todo init
todo project attach todo --path /path/to/this/project

todo add "Dogfood the first workflow" --tag goal/todo/v0_1
todo ls --tree
todo start <id>
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

On a second machine, clone the sync remote first, because `todo sync init`
creates an empty repository with no remote when pointed at an empty directory:

```sh
git clone <sync-remote-url> /path/to/todo-ledger
todo sync init --repo /path/to/todo-ledger
todo sync
```

`todo sync` pulls, imports every envelope this machine does not have, rewrites
the canonical log, commits, and pushes. Concurrent appends from two machines are
merged by the `merge=union` driver and deduplicated by CID, so the log never
conflicts and replay converges. The database itself never syncs.

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
backup workflows, including a safe isolated ledger for a fresh dogfooding run.

For a practical mapping from weekly Obsidian task notes into global and
project-scoped work, see [the weekly-notes tutorial](docs/weekly-notes-tutorial.md).

## Current direction

Development proceeds from one working local global ledger, to interaction with
one-to-three repositories, and then to personal/shared spaces and multiple
users. Exact events can be synchronized through an explicit Git checkout; each
device keeps its database as a rebuildable local view. New writes use the v14
payload protocol and import accepts v5 through v14. See
[DI-0007](DI/DI-0007-staged-global-to-shared-rollout.md) for the staged rollout
and [DI-0007-git-jsonl-sync](DI/DI-0007-git-jsonl-sync.md) for the transport.

Multi-user access and hosting, whether a sync server, a browser build, or a
signed native distribution, are deferred until PromiseGrid reaches 1.0. The
routes and the two questions that must be settled first are recorded in
[TE-0014](docs/thought-experiments/TE-0014-multi-user-access-and-hosting.md).

The current PromiseGrid and `todo-report` comparison is recorded in
[the integration review](docs/integration-review-2026-09-04.md). Direct Grid
reuse and a possible Go migration remain open under
[DR-0002](DR/DR-0002-promisegrid-runtime-and-language.md). Editable Markdown,
including a possible `TODO/TODO.md` layout and readable stable IDs, is deferred
under [DR-0003](DR/DR-0003-editable-markdown-identity.md).

The latest hands-on command review and prioritized usability gaps are recorded
in [the CLI UX audit](docs/cli-ux-audit-2026-09-04.md).

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

## v0.2 model

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
```

Item references may be full UUIDs or a unique trailing fragment printed by the
CLI. Outside an attached directory, items enter the global ledger without a
project. Use `--project todo` to override directory inference, and `todo ls
--global` to see all open items across project scopes.

Hierarchical namespace examples include `project/todo`, `goal/todo/v0_1`,
`role/developer`, and `topic/storage/turso`. Their paths can be renamed with
`todo tag mv`; assignments retain stable internal IDs and the old path remains
an alias.

See [the versioned tutorial](docs/tutorial.md) for complete v0.1 capture and
v0.2 focus workflows.

# Tutorial: From Capture to Portable Sync

This tutorial follows the product in release order. Each section introduces
only the concepts available in that version and explains how the underlying
ledger records them.

## v0.1: Capture, organize, and preserve context

### 1. Install and initialize an isolated dogfooding ledger

For a restart, choose new, empty directories instead of removing or reusing an
existing ledger. These environment variables apply only to the current shell:

```sh
./dev install
export PATH="$HOME/.cargo/bin:$PATH"
export TODO_CONFIG_HOME="$HOME/todo-dogfood/config"
export TODO_DATA_HOME="$HOME/todo-dogfood/data"
todo init
```

`./dev install` creates the executable. `todo init` separately creates one
global ledger and its configuration in the selected directories. Items are not
stored in each source repository. Keep these exports in your dogfooding shell or
shell profile; omit them only when you intentionally want the normal default
ledger locations.

### 2. Attach working directories to project scopes

```text
todo project attach cdint --path ~/dev/cdint
todo project attach todo --path ~/dev/exocortex/todo_app_project
```

The longest matching directory mapping wins. An explicit `--project cdint`
always overrides inference. Outside a mapped directory, capture goes to the
unscoped global inbox.

```text
todo ls             # current project, or unscoped inbox
todo ls --global    # open items across every scope
```

### 3. Capture first; commit later

```text
todo add "Trace protocol SID encoding" --tag topic/promisegrid/protocol-sid
todo start <id>
todo done <id>
```

New items start in `inbox`. `start` records the state `active`.

### 4. Build structure without losing source context

```text
todo add "PromiseGrid fundamentals"
todo add "Study RFC 8949" --parent <parent-id> --tag topic/promisegrid/cbor
todo note <id> "Concentrate on deterministic encoding"
todo cite <id> /path/to/source-note.md --label "source note"
todo ls --tree
```

Parents express decomposition. Hierarchical namespace paths classify the item.
Notes and citations append context instead of rewriting history.

```text
[ ] PromiseGrid fundamentals
  [ ] Study RFC 8949  topic/promisegrid/cbor
```

Namespace paths can change without invalidating assignments:

```text
todo tag mv topic/promisegrid/cbor topic/promisegrid/encoding
todo tag ls --tree
```

### 5. Inspect the ledger history

```text
todo show <id> --history
```

Every mutation stores an exact Grid-CBOR envelope plus queryable local fields.
The proof slot is deliberately `null` in v0.1.

### 6. Failure safety

Initial title, tags, and parent membership are created in one transaction. A
bad tag or unknown parent reports an error without leaving a partial item:

```text
todo add "Bad capture" --tag INVALID
todo add "Bad capture" --parent unknown
```

## v0.2: Select actionable work

v0.2 keeps the v0.1 capture model and adds explicit facts used to decide what
is actionable. Existing ledgers migrate in place when first opened.

### 1. Add priority and dates

```text
todo priority <id> high       # aliases: p0 through p3
todo due <id> 2026-09-04
todo defer <id> 2026-09-01
```

Priorities are `critical`, `high`, `normal`, and `low`; `normal` is the
default. Dates deliberately use unambiguous `YYYY-MM-DD` input. Clear dates
explicitly:

```text
todo due <id> --clear
todo defer <id> --clear
```

A future-deferred item is not ready. It also cannot be started until the defer
date is cleared or reached; this prevents contradictory “active but unavailable”
state.

### 2. Distinguish decomposition from execution order

```text
Build PIH prototype                 parent/child structure
├── Get repository access
└── Implement Go spike
      blocked by -> Get repository access
```

The tree alone does not assert that one child blocks another. Add that fact
separately:

```text
todo block <implementation-id> --on <access-id>
todo unblock <implementation-id> --on <access-id>
```

Dependencies must remain within one project scope and cannot form cycles. A
done or canceled blocker satisfies the dependency; the relationship remains
recorded until explicitly removed.

### 3. Ask what is ready

```text
todo ready
todo --project cdint ready
todo ready --global
```

Readiness is a filter, not a score:

```text
open state
AND defer date is absent, today, or past
AND every blocker is done or canceled
```

### 4. Ask for one deterministic next item

```text
todo next --explain
```

The policy compares ready items lexicographically:

```text
active before inbox
-> critical, high, normal, low
-> dated before undated; earlier due date first
-> older UUIDv7 item first
```

Example:

```text
[ ] 429e570b Implement PIH spike  !critical due:2026-09-04 depends:1
why:
  ready: no incomplete blockers and defer date reached
  state: inbox
  priority: critical
  due: 2026-09-04
```

`depends:1` reports declared dependency edges, including already-satisfied
ones. `next` never silently asks an LLM to choose; a later advisor can record a
suggestion without changing this deterministic policy.

## v0.3: Back up before dogfooding

Create each backup at a new path:

```text
todo backup /path/to/backups/todo-2026-09-04
```

The directory contains `events.cbor` with every exact immutable event,
`items.json` with the rich current view, and one importable Markdown tree for
the global inbox and each non-empty project. Follow its generated `RESTORE.md`
commands to recover titles, lifecycle states, project scopes, and hierarchy in
a clean ledger. IDs and richer facts are retained as evidence in JSON/CBOR but
are not yet restored by the Markdown path.

New events use the v5 Grid-CBOR protocol while existing v1-v4 event bytes and
CIDs remain valid. This is an event-format transition, not a database reset.

## v0.4: Synchronize exact v5 events through Git

The portable record is a dedicated Git checkout containing a versioned
`ledger.toml` and canonical `events.jsonl`. The local `ledger.db` remains a
machine-private query view and must not be copied through Git, Syncthing, or a
cloud file synchronizer.

### 1. Create or clone the sync repository

Use a normal Git checkout with a branch checked out. Do not point `todo` at an
empty `.git` directory whose remote has existing commits.

```text
git clone <private-ledger-remote> ~/dev/todo-ledger
```

On the first machine, configure the checkout and publish local events:

```text
todo sync init --repo ~/dev/todo-ledger
todo sync
```

`todo sync init` creates `ledger.toml`, `.gitattributes`, and `events.jsonl` if
they do not already exist. `todo sync` imports remote events, canonicalizes the
complete local event set, commits a changed log, and pushes it. It is explicit:
run it when you want to exchange work, rather than relying on a background
process.

### 2. Join the ledger from another machine

Clone the same remote locally, initialize that machine's database, then connect
the checkout and synchronize:

```text
git clone <private-ledger-remote> ~/dev/todo-ledger
todo init
todo sync init --repo ~/dev/todo-ledger
todo sync
```

The second `todo sync` imports the exact event history and rebuilds its local
views. New v5 events include a persistent local actor identifier. v1-v4 events
remain preserved historical evidence but are not yet replayable by this first
portable-sync implementation.

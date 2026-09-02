# Tutorial: From v0.1 Capture to v0.2 Focus

This tutorial follows the product in release order. Each section introduces
only the concepts available in that version and explains how the underlying
ledger records them.

## v0.1: Capture, organize, and preserve context

### 1. Install and initialize

```text
cargo install --path crates/todo-cli
todo init
```

Installation creates the executable. `todo init` separately creates one global
ledger and its configuration. Items are not stored in each source repository.

### 2. Attach working directories to project scopes

```text
todo project attach cdint --path ~/dev/cdint
todo project attach todo --path ~/dev/omnicortex/todo-app
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
todo ongoing <id>
todo done <id>
```

New items start in `inbox`. `ongoing` is an alias for `start`; both store the
single state `active`.

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

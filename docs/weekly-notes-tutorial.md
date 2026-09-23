# Structuring Weekly Notes in the Todo Ledger

This tutorial is based on the recurring structure in the W32-W35 periodic
notes and the daily and weekly Obsidian Tasks queries. It does not import or
modify those notes. The examples are illustrative so the ledger can be
dogfooded deliberately instead of receiving a bulk dump.

## Current Reality

The application has one global event ledger with optional project scopes. It
can attach multiple repository directories to projects and explicitly
synchronize exact v5 events through a dedicated Git checkout. It does not read
repository TODO files or pin remote GitHub task sources.

On macOS, the default paths selected by the `dirs` crate are:

```text
~/Library/Application Support/todo-ledger/config.toml
~/Library/Application Support/todo-ledger/ledger.db
```

`TODO_CONFIG_HOME` and `TODO_DATA_HOME` override those directories. On this
machine the default directory does not currently exist and `todo` is not on
`PATH`, so only isolated development/test ledgers have been exercised so far.

`../todo-ledger` can be configured as the dedicated Git checkout with `todo
sync init --repo ../todo-ledger`. It holds the versioned exact event log, not
the live database. Run `todo sync` explicitly on each machine to exchange and
merge event sets.

Do not place the live `ledger.db` or its WAL under Git, Obsidian Sync, Dropbox,
or another file synchronizer. Copying a live SQLite-family database does not
provide event merging and can corrupt or silently fork state. Current backups
are safe local recovery artifacts. Exact archive replay is available through
the event-log importer; synchronization remains an explicit user action.

## The Components

### Ledger

The ledger is the global durable history. Every mutation appends an immutable
Grid-CBOR event and updates relational views used for queries. There is one
ledger, not one database per repository.

### Item

An item is durable work with a stable UUID, title, lifecycle state, priority,
dates, and relationships. The CLI prints an eight-character trailing fragment
that can be used while it remains unique.

### Project

A project is an execution scope inside the global ledger. A directory mapping
selects a project automatically when commands run inside that directory. It
does not make the repository authoritative for the tasks.

### Parent

A parent edge means structural decomposition: the child is part of the parent.
Parents and children must currently be in the same project scope. Parent edges
are not execution dependencies.

### Blocker

A blocker edge means one item cannot become ready until another item is done or
canceled. Blocking is separate from hierarchy and must remain acyclic within a
project.

### Namespace

Namespaces classify work without changing its structural parent. Useful kinds
for the weekly notes include:

```text
goal/research/preprint
role/lab
topic/finance
topic/vehicle
context/home
context/computer
```

Paths are human-facing names backed by stable namespace IDs. Renaming a path
does not destroy assignments.

### Note and citation

A note appends progress or reasoning. A citation attaches a source file, URL,
paper, issue, or decision. These are better homes for the prose and links that
sit beside many weekly tasks than expanding the task title.

### Views

`ls`, `ready`, `next`, Markdown, and JSON are views of the ledger. Weekly and
daily pages should eventually be generated or managed views, not duplicate
authoritative copies of the same tasks.

## Translating Existing Note Data

| Weekly-note data | Ledger representation | Current limitation |
| --- | --- | --- |
| `- [ ]`, `- [>]`, `- [x]`, `- [-]` | inbox, active, done, canceled | Supported |
| Indented checkbox | structural parent | Supported within one scope |
| Heading such as a repository or research effort | project or project root item | Supported |
| Subject heading such as health or finance | hierarchical namespace | No tag-filter CLI yet |
| Due date | `due` | Supported |
| Scheduled/start date | `defer` only when it means “not available before” | Separate scheduled/start semantics absent |
| Priority marker | `priority` | Supported |
| “after X” or graph arrow | blocker edge | Supported within one project |
| Markdown or external URL | citation | Supported |
| Progress prose | append-only note | Supported |
| Completion date | state-change event timestamp | Not shown as a dedicated completion field |
| Parked/someday | namespace plus low priority | Still appears in `ready`; true parking is absent |
| Repeating obligation | repeated manual item | Recurrence absent |
| Weekly selection | query/view over durable items | Saved weekly views absent |

The distinction between `due` and `defer` matters:

```text
due    = the consequence or commitment date
defer  = do not offer this work before this date
```

A calendar appointment is not necessarily either one. The current model should
not force every scheduled event into a due date.

## Suggested Initial Shape

Keep personal obligations without a repository in the global inbox. Use
projects for durable execution contexts, especially repositories. Use
namespaces for themes that cross projects.

```text
global inbox
|-- life administration
|-- household and vehicle
`-- one-off health actions

project/agents
`-- improve reusable agent instructions
    |-- review the next library design
    |-- migrate accepted modules
    `-- dogfood in several repositories

project/research
`-- produce the first analysis
    |-- establish the data foundation
    |-- reproduce one anchor result
    |-- query concept maps
    `-- run the main analysis

project/todo
`-- dogfood the ledger
    |-- harden the CLI
    |-- implement exact recovery
    `-- prototype graph visualization
```

Do not recreate every durable item each week. A weekly review should choose
from existing open items, add genuinely new work, correct stale facts, and
record progress.

## Foundation-First Walkthrough

The sequence below follows the implementation layers: ledger, scope, structure,
context, lifecycle, focus, repair, and interchange.

### 1. Install and initialize the one global ledger

```sh
./dev install
export PATH="$HOME/.cargo/bin:$PATH"
todo init
```

Initialization creates global configuration and local storage. It does not
install the executable and does not create files in every repository.

### 2. Capture unscoped obligations

Run outside an attached directory:

```sh
todo add "Arrange vehicle maintenance" --tag topic/vehicle
todo add "Review insurance options" --tag topic/finance
todo ls
```

These are global-inbox items because no project won scope resolution.

### 3. Attach repository directories

```sh
todo project attach agents --path ~/dev/cdint/Agents
todo project attach todo --path ~/dev/exocortex/todo_app_project
todo project attach research --path ~/dev/research/example
todo project ls
```

Inside an attached directory, ordinary commands select that project. An
explicit project always wins:

```sh
todo --project agents ls
todo ls --global
```

This is already a multi-repository mapping, but not repository integration.
The repositories do not yet contain ledger pointers or synchronized task
projections.

### 4. Build a project-local hierarchy

```sh
todo --project research add "Produce the first analysis" --tag goal/research/preprint
todo --project research add "Establish the data foundation" --parent <root-id>
todo --project research add "Reproduce one anchor result" --parent <root-id>
todo --project research ls --tree
```

Use hierarchy for decomposition, not for category labels and not merely because
one task must happen first.

### 5. Preserve supporting context

```sh
todo note <item-id> "The existing pipeline should be inspected before rebuilding it"
todo cite <item-id> README.md --label project-plan
todo cite <item-id> https://github.com/OWNER/REPO/blob/COMMIT/TODO.md --label pinned-source
todo show <item-id> --history
```

A GitHub URL containing a commit SHA is readable and immutable, but today it is
only a citation. The ledger does not fetch, verify, or import it.

### 6. Record execution facts

```sh
todo priority <item-id> high
todo due <item-id> 2026-12-01
todo defer <item-id> 2026-09-14
todo block <analysis-id> --on <foundation-id>
```

Avoid assigning speculative dates to every item. The notes already show the
value of leaving research work undated until a real commitment exists.

### 7. Work through the lifecycle

```sh
todo start <item-id>
todo note <item-id> "Validated the input dimensions"
todo done <item-id>
todo reopen <item-id>
todo cancel <item-id>
```

Cancellation means the work is no longer intended. It should not be used as a
substitute for “someday.”

### 8. Generate daily and weekly attention views

```sh
todo ready
todo next --explain
todo --project research ready
todo ready --global
todo ls --all
```

`ready` implements the useful core of the existing daily query: open, no longer
deferred, and free of incomplete blockers. `next` then orders ready work by
active state, priority, due date, and age. Dedicated overdue, completed-today,
tag-grouped, and scheduled-today views are not implemented.

### 9. Correct mistakes without rewriting history

```sh
todo edit <item-id> "Corrected title"
todo parent <item-id> --clear
todo tag rm <item-id> topic/old-name
todo tag mv topic/old-name topic/new-name
```

These commands append repair events. Project moves are guarded when structural
or blocking relationships would be broken.

### 10. Import a deliberately prepared tree

The existing weekly notes should not be imported wholesale. First make a small
project-specific Markdown file whose first task is the project root:

```markdown
- [ ] Produce the first analysis
  - [ ] Establish the data foundation
  - [ ] Reproduce one anchor result
  - [ ] Query concept maps
  - [ ] Run the main analysis
```

Preview and then import it:

```sh
todo --project research import markdown research-plan.md --dry-run
todo --project research import markdown research-plan.md --cite-source
```

The import is atomic and the citation preserves where the batch came from. It
creates new item IDs and does not parse Obsidian date, priority, or recurrence
syntax.

### 11. Export and back up

```sh
todo export markdown --global --all --output all-tasks.md
todo export json --global --all --output all-tasks.json
todo export cbor --output all-events.cbor
todo export log --output events.jsonl
todo backup /path/to/new/todo-backup-2026-09-04
```

Markdown is readable and partially restorable. JSON is a rich current view.
CBOR and the event log preserve exact event bytes. To synchronize v5 events,
configure a dedicated Git checkout once with `todo sync init --repo PATH`, then
run `todo sync` explicitly on each machine. The command imports remote events,
publishes local events, and uses Git's union merge for concurrent appends.

## Global Parent with Project Children

The requested shape is conceptually useful:

```text
global: Build the broader research system
`-- project/research tasks from a repository
```

It is not representable as parenthood today because parent and child must share
a project. This invariant keeps project trees movable and understandable.

Current options are:

1. Put the umbrella and all descendants in the same project.
2. Keep one root per project and assign the same `goal/...` namespace to related roots.
3. Keep the global umbrella as an index item with citations to project plans.

The best future design is a typed cross-scope relation such as `contributes-to`
or `implements`, not cross-scope structural parenthood. A later import command
could accept `--relates-to <global-id>` while preserving project-local trees.
The graph view could render this relation directly.

## GitHub-Readable and Pinned Project Lists

The Agents/Mogent model demonstrates a useful pattern: a source declaration is
separate from a resolved immutable pin. Applying that lesson here requires
clear authority boundaries:

```text
global event ledger                    authoritative task history
project TODO/TODO.md                   readable managed projection
projection manifest                   ledger ID, event head, format version
remote pin                            repository URL, commit, path, content hash
```

A safe pull flow would be:

```text
fetch pinned file
-> verify commit and content hash
-> match stable item identities
-> show semantic additions and changes
-> approve
-> append ledger events
```

A safe publish flow would regenerate the managed projection from a known ledger
head. Missing Markdown lines must never silently delete tasks. DR-0003 leaves
the exact stable-ID representation open until real editing pressure clarifies
whether readable slugs, block IDs, links, or a sidecar are best.

This capability is not implemented. A commit-pinned GitHub citation is the
honest current substitute.

## A Lightweight Native Graph Viewer

A graph prototype is straightforward because the domain already distinguishes
node identity and two important edge types. The renderer should consume a
snapshot adapter rather than query Turso tables directly.

Recommended first stack:

```text
Rust
petgraph          graph structure and traversal
macroquad         small native window, input, and 2D drawing
custom force layout for the first few hundred nodes
```

`macroquad` is substantially lighter than Tauri or a browser application and
does not require JavaScript. A simple force-directed layout needs repulsion,
spring forces for edges, damping, dragging, zooming, and labels. An O(n²)
repulsion pass is adequate for early dogfooding; spatial partitioning can wait.

Visual vocabulary should preserve domain meaning:

```text
solid edge        structural parent
arrow edge        blocked-by
dashed edge       future cross-scope relation
node color        lifecycle state
node border       project
node size         priority
faded node        deferred
```

Start with 2D. It is easier to read, label, select, and compare over time. A
read-only 2D prototype fed by `todo export json --global --all` is roughly a
one-to-three day slice. Filters, stable layouts, details, and live refresh make
it closer to one or two weeks.

For a later 3D experiment, `kiss3d` is a relatively direct native Rust option.
A basic point-and-edge scene is a few days, but useful labels, selection,
occlusion control, and navigation take longer. Three dimensions provide more
space but usually make task dependencies harder to understand, so they should
be an alternate view over the same graph snapshot rather than the initial UI.

The first graph-specific product change should be a stable snapshot containing
items, project names, namespaces, and typed edges. That keeps visualization
read-only and prevents an attractive prototype from defining storage or task
semantics accidentally.

## Practical Starting Point

Begin with three small roots rather than importing every note:

1. One global life-administration root with a few current obligations.
2. One attached software project with a project-local implementation tree.
3. One attached research project with foundation, anchor, and analysis phases.

Use the ledger for durable facts and history. Continue using weekly notes as a
human review surface until the ledger has the missing scheduled, parked,
tag-query, and saved-view semantics. This keeps early structure coherent while
dogfooding reveals which projection and synchronization features are genuinely
necessary.

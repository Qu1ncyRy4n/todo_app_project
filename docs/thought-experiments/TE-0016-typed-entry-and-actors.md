# TE-0016: Typed Entry, Actors, and Namespace Records

Date: 2026-10-14
Status: direction agreed with the developer. Implemented: typed entry, actors,
fields, membership, roles, `relates-to` (protocol v14, `93c4ac6`, `ce7d4ac`) and
projects keyed by `repo`. Not implemented: emoji forms, project default tags,
GUI quick entry. The TOML manifest is implemented (`cb678af`).
Items: `farag-susot`, `nafun-rinan`, `popal-nofah`, `jamim-mamin`
Research: `docs/research/task-field-survey.md`

## Decision Under Test

How can typed, hierarchical metadata (tags, projects, people, roles, links) be
written on the fly, in one line, in both the CLI and the GUI, without a
metadata editor?

The motivating constraint, in the developer's words: if input requires a
metadata editor, it is not going to happen.

## Primitives

No new primitive. The three from `DESIGN-QUESTIONS.md` become concrete:

| Primitive | Meaning |
|---|---|
| Node | an item, or a namespace node (`work/cdint`, `quincy`, `project/todo`) |
| Link | typed: item→item (`depends`, `relates-to`, parent) or item→namespace (`tag`, `by`) |
| Field | `key=value` on a node: due, estimate, email |

A tag is one kind of item→namespace link. Time is not a primitive: it is fields
(due, defer, scheduled) plus the event log (created and done derive from
`recorded_at`).

The only model addition is **fields on namespace nodes**, an open string map
so a new key never needs a protocol version.

## Typed entry syntax

`kind:path` links an item to a node. `key=value` sets fields. The same parser
serves CLI arguments and the GUI quick-entry bar.

```sh
todo add "Fix sync race" tag:work/apps/todo-app/gui by:quincy see:jamim-mamin
todo set quincy email=… gh=Qu1ncyRy4n
todo set cdint member=quincy member=steve
```

Each field has a slug form (always on) and optional ASCII and emoji forms,
enabled in configuration. Emoji match Obsidian Tasks exactly so Obsidian task
lines import directly.

| Field | slug | ascii | emoji |
|---|---|---|---|
| tag | `tag:a/b` | `#a/b` | — |
| project | `in:todo` | `+todo` | — |
| author | `by:quincy` | `@quincy` | — |
| due | `due:2026-10-20` | — | 📅 |
| defer | `defer:…` | — | 🛫 |
| priority | `p:high`, `p1` | `!1` | ⏫ 🔼 🔽 |
| estimate | `est:30m` | `30m` (off by default) | — |
| depends | `dep:<id>` | — | ⛔ |
| relates | `see:<id>` | — | 🔗 |
| id | proquint | — | 🆔 |

Choices: `@` is a person, not a date, unlike Super Productivity. Bare `30m` is
off by default because it misfires on titles. Avoid `|` and `{a,b}` in the
grammar because the shell consumes them. `#` is acceptable in the shell.

Later idea, not adopted: arrow ligatures for dates, such as `=|` due, `=>`
defer, `|=` started.

## Actors

The umbrella word is **actor**: anyone who can act, whether a person, an org,
or an agent. It already matches `actor_id` on events. An actor is a namespace
node with a `kind` field.

- Identity for now is a **local alias**: a plain first name or GitHub handle,
  such as `quincy`, carrying fields such as `email` and `gh`. No global ID
  scheme until PromiseGrid identity arrives (`grid_id` is reserved).
- `me` is a configured alias for the operator, so `by:me` works.
- Suggested keys: person `name email gh url grid_id`; org `name url gh
  grid_id`; agent `kind scope note`.
- Contact fields reach the Git sync repository. Keep that remote private.

### Membership is a graph, not a tree

A person can belong to many orgs, so membership is a link (`member-of`), not
nesting. Attribute work to the person who did it. Org credit is derived
through membership unless the org is named directly (`by:cdint`). Agents have
exactly one owner, so they may nest under it: `quincy/agent/<id>`.

### Roles

Four core link kinds: `by:` author, `contrib:` contributor, `to:`
assignee/delegate, `for:` requester. Open roles (lead, maintainer, reviewer,
editor, technician…) use `as:<role>/<actor>`, for example
`as:reviewer/steve`. Role names live under `role/` and are created on first
use. A role is a property of the link, never of the actor's path, so one actor
holds different roles on different items and projects without duplication.

## Projects and tags

Today the developer marks projects with tags (`tag/work/apps/todo-app`) and
does not attach directories. The direction is that **a project is a namespace
node with a `repo` field**:

- `in:todo` sets an item's single project. Tags remain many.
- Directory inference matches `git remote get-url origin` against project
  `repo` fields, instead of absolute paths in `config.toml`. Repositories are
  declared uniformly across machines with Nix, so remotes are the portable key.
- A project node can carry default tags that its items inherit.

## Editable manifest: TOML for now

Decided 2026-10-19: TOML. It adds no dependency (`config.toml` already uses
it) and is good enough for flat records. KDL, HOCON, and UCL were set aside;
the developer liked none of them. Mogent's small language (below) is the
likely long-term syntax once it settles.

The manifest is for reviewing and bulk-editing actor and project records. It is
optional: `todo set` and `todo project repo` write the same events one record
at a time. The pipeline follows TE-0009: compare the file with the ledger, show
the events, then write them.

```toml
# Written by `todo manifest export`; edit, then `todo manifest apply`.
based_on = "3f9c0e1a72b4d856"   # digest of the records at export

[actor.quincy]
gh = "Qu1ncyRy4n"
kind = "person"
member-of = ["cdint"]

[actor.cdint]
kind = "org"

[project.todo]
repo = "github.com/Qu1ncyRy4n/todo_app_project"
```

- `todo manifest export [FILE]` writes every actor and project; `todo manifest
  apply FILE [--dry-run]` prints the changes as `+`/`-` lines and writes them.
- A listed node is authoritative for its fields and `member-of`: a removed line
  clears that field or link. An unlisted node is untouched. Nodes are never
  deleted. The list key is `member-of`, matching `todo set x member-of=y`;
  `member_of` could collide with a field, since keys allow `_`.
- **Stale-file check (from Mogent's input hashes):** `based_on` is a short
  SHA-256 of the actor and project records at export, not the ledger head, so
  adding ordinary items never makes the file stale. If the records have changed
  since and the file would change anything, apply refuses, so an old file cannot
  revert newer `todo set` edits or records synced from another machine. After a
  successful apply only the `based_on` line is rewritten; comments and layout
  stay. A hand-written file without `based_on` is applied as is.
- Ceiling: `member-of` names actors. A link to a non-actor would export as a
  full path that does not parse back; nothing creates one today.
- Comment out a line with `#` to disable it; TOML has no node-level disable.

### Ideas kept from Mogent's language

Reviewed 2026-10-19 in `~/dev/cdint/Agents` (`docs/proposals/MOGENT-FIRST-SLICE-SPEC.md`,
`syntax/`). It is a small block/literal language: HCL-shaped, but no HCL
expressions, closed schemas, `//` comments, retained source ranges and comments,
a deterministic formatter. Worth adopting when it is stable and has shared
conformance cases, by porting its generic lexer/parser (about 650 lines of Go)
to a small Rust crate. Ideas to keep regardless of syntax:

- **Type aliases:** `person "quincy" { … }` normalizing to
  `actor "quincy" { kind = "person" }`.
- **Child lists:** `org "cdint" ["quincy", "steve"]` for membership.
- **Plan, then apply:** show the exact events before writing any.
- **Closed schemas with located errors:** an unknown key is an error naming its
  line, never ignored.
- **Input records:** the `based_on` check above.
- **Byte-exact no-op edits:** rewriting one value keeps the rest of the file.

Not adopted: `let` bindings, interpolation, recipes, multiline text. Actor
records do not need them. Mogent's README grants no license, so porting its code
needs the owner's permission; writing a parser from the spec avoids copying code.

## Relation catalog

Adopt now: `relates-to`, **symmetric** (stored once, shown on both items; A→B is
refused when B→A exists). Needs protocol v14. Input alias `see:`.

Documented, not adopted until a real case appears:

| Relation | Direction | Use |
|---|---|---|
| `duplicates` | directed | close one item in favour of another |
| `supersedes` | directed | an old plan replaced by a new one |
| `spawned-from` | directed | a follow-up created from an item or note |
| `leads-to` | directed | soft sequencing that does not block readiness |
| `member-of` | actor→actor | org membership (needed with actors) |

## Open

- Whether `kind` constrains containment beyond "agents nest under one owner".
- Whether to re-enumerate TE/DR/DI records with proquint IDs.

Recorded by agent-assistant.

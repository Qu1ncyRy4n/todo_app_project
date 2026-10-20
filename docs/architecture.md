# Architecture

Status: implemented through v0.3.0 by DI-0001 through DI-0009. DI-0007 sets the
staged local-to-shared product direction; its later stages are not implemented
and are deferred pending PromiseGrid 1.0 under TE-0014. DI-0008 and DI-0009 add
PromiseGrid framing compatibility and dogfood recovery.

## Global space and project scopes

One global database owns all events. An item may have no project, in which case
it belongs to the global inbox, or it may reference one project scope. Global
configuration maps absolute directory roots to project IDs. Resolution uses:

```text
explicit --project
  -> longest matching configured directory root
  -> project whose `repo` field matches `git remote get-url origin`
  -> configured default project
  -> global space
```

Directory roots are per machine and stay in `config.toml`. The `repo` field is
a namespace field (protocol v14), so it syncs with the ledger and every clone of
the repository, on any machine, infers the same project. `todo project repo
<name> [url]` records it, defaulting to the current checkout's `origin`. Remotes
are compared as `host/owner/name`, so SSH and HTTPS forms match. Two projects
naming the same repository is an error that asks for `--project`.

## Durable records and local views

The durable record is a ledger Grid message:

```text
grid([42(pCID), parents, payload, proof])
```

The proof slot remains `null`, and semantic validity is checked before an event
is persisted. Historical events retain their exact bytes and original CIDs. New
writes use the v14 payload protocol over the registered Grid-CBOR codec
(`0x1027`) with the generic envelope validation from DI-0008, and import accepts
v5 through v14. The payload protocols are frozen, content-addressed files under
`docs/protocols/`; a protocol is never edited in place, because its bytes are
hashed into every event that references it. Adding a field means adding a file.

Turso stores the exact envelope bytes and extracted fields used to build local
views. Exact CBOR archives are the durable exchange format. JSON and Markdown
backups are lower-fidelity, CLI-restorable safety layers; exact replay/import
remains planned.

## User-facing lifecycle

```text
inbox -> active -> done
            \-> canceled
done/canceled -> active (reopen)
```

`start` moves an item to `active`. Capturing an item is not making a
Promise-Theory promise. Promise, observation, evidence-policy, delegation, and
multi-observer assessment commands remain future protocol families.

## Actionable focus

Structural parents organize decomposition. Blocking dependencies independently
constrain execution and must remain acyclic within a project scope.

```text
ready = open
     && defer_until <= today (or absent)
     && every blocker is done or canceled

next = ready ordered by state, priority, due date, then UUIDv7 age
```

The ordering is lexicographic and returned as an explanation. It is a domain
policy shared by CLI and future clients, not an opaque CLI score.

## Namespaces

Namespace entries have stable IDs, a kind, one canonical slash-separated path,
and aliases. Assignments refer to the stable ID. Paths support segment-aware
ancestor checks. The initial kinds are
`tag`, `project`, `role`, `goal`, `topic`, and `context`; other validated kinds
are representable in the domain model. v0.2 does not load executable plugins.

## Synchronization

The interchange artifact is `events.jsonl` in a Git checkout the operator names
explicitly: one line per envelope, carrying the CID and the exact bytes, sorted
by CID and deduplicated. Any union of the same event set therefore produces
identical file content.

```text
todo sync
  -> git pull --no-rebase   (when an upstream is set)
  -> import unknown CIDs into the local ledger
  -> rewrite events.jsonl canonically
  -> commit, push; on rejection pull, re-import, re-canonicalize, push once
```

`.gitattributes` marks `events.jsonl merge=union`, so concurrent appends from
two machines concatenate instead of conflicting, and CID deduplication makes the
concatenation harmless. SQLite never syncs; it is rebuilt from events per
DI-0002.

Convergence is guaranteed; agreement is not. Two machines editing the same field
both keep their events, and the projection resolves by `(recorded_at, event_id)`,
which is last-writer-wins per field with the fork preserved in `parent_cid`.
Nothing reports that this happened. See TE-0014.

## Graph editor

`todo-gui` is a second client over the same ledger, not a second authority. It
holds no state the ledger cannot rebuild, and every mutation goes through the
same `Ledger` methods the CLI calls.

```text
todo-core   domain types, events, CIDs, proquints   (no platform dependencies)
todo-store  Turso projection, configuration         (turso, dirs, tokio)
todo-gui    eframe window over todo-store           (37 Ledger call sites)
  geometry.rs   pure layout math, unit tested without a window
  theme.rs      every appearance knob, loaded from theme.toml
  palette.rs    tag colors, rainbow and hue colormaps
  tags.rs       filter tri-state and tag inheritance
```

Layout math lives in `geometry.rs` as pure functions over positions and
rectangles, so arrangement is testable without rendering. Appearance lives in
`theme.toml` with `deny_unknown_fields`, so a typo is reported rather than
silently ignored. Node placement lives in `gui-layout.json`. Neither file holds
work; deleting them loses arrangement only.

`todo-core` is the only crate that builds for `wasm32` today. That is a
deliberate property, not an accident, and it is what a browser build would
depend on. See TE-0014.

## Extension boundary

The safe initial extension surface is data-driven:

- versioned payload protocol modules;
- registered namespace kinds;
- import/export adapters;
- deterministic view and ranking policies.

Executable plugin loading is deferred until permissions, versioning, failure
isolation, and migrations have a concrete use case.

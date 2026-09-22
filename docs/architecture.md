# Architecture

Status: implemented through v0.3.0 by DI-0001 through DI-0009. DI-0007 sets the
staged local-to-shared product direction; its later stages are not implemented.
DI-0008 and DI-0009 add PromiseGrid framing compatibility and dogfood recovery.

## Global space and project scopes

One global database owns all events. An item may have no project, in which case
it belongs to the global inbox, or it may reference one project scope. Global
configuration maps absolute directory roots to project IDs. Resolution uses:

```text
explicit --project
  -> longest matching configured directory root
  -> configured default project
  -> global space
```

The mapping stays in global configuration rather than placing control files in
every repository. A later project-local pointer format can be added if
dogfooding shows the global mapping is awkward.

## Durable records and local views

The durable record is a ledger Grid message:

```text
grid([42(pCID), parents, payload, proof])
```

The proof slot remains `null`, and semantic validity is checked before an event
is persisted. Historical v1-v3 events retain their exact bytes and raw-codec
CIDs. New v5 events use the registered Grid-CBOR codec (`0x1027`) and generic
envelope validation from DI-0008. The payload protocols are frozen,
content-addressed files under `docs/protocols/`.

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

`ongoing` is a CLI alias for `active`. Capturing an item is not making a
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

## Extension boundary

The safe initial extension surface is data-driven:

- versioned payload protocol modules;
- registered namespace kinds;
- import/export adapters;
- deterministic view and ranking policies.

Executable plugin loading is deferred until permissions, versioning, failure
isolation, and migrations have a concrete use case.

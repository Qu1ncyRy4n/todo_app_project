# Architecture

Status: implemented through v0.2 by DI-0001 through DI-0005.

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

The durable record is a ledger Grid-CBOR message:

```text
grid([42(pCID), parents, payload, proof])
```

The v0.1 proof slot is `null`. The format therefore borrows Grid framing but
does not claim PromiseGrid compatibility, identity, or authentication. The
payload protocols are frozen, content-addressed files under `docs/protocols/`.
v0.1 event kinds retain the v1 pCID; v0.2 focus kinds use the v2 pCID.

Turso stores the exact envelope bytes and extracted fields used to build local
views. Exact CBOR is the durable exchange unit. JSON and plain-text
import/export are planned adapters, not part of v0.2.

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

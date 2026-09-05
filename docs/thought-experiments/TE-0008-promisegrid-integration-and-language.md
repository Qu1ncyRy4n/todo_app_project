# TE-0008: PromiseGrid Integration and Implementation Language

Date: 2026-09-04
Status: open

## Pressure

The todo ledger is implemented and tested in Rust. The current private Grid
work is implemented in Go and now provides a reusable generic envelope plus a
planned shared CAS, VCS, reference-set, and synchronization library. Direct use
could prevent incompatible duplicate machinery, but changing languages or
binding across them creates migration and operational costs.

The product should progress in three stages:

1. one working local global todo ledger;
2. one global ledger interacting with one, two, then three repositories; and
3. personal space, shared space, and multiple users.

## Routes

### A: Keep Rust and implement compatible Grid primitives

Normal use remains simple and preserves typed identifiers, exhaustive enums,
ownership checks, and the existing test suite. Cross-language conformance
fixtures can prove wire compatibility. The cost is duplicate implementation of
Grid framing, replay, CAS, and eventually synchronization.

Failure isolation is good because no foreign runtime is involved. Migration is
incremental: legacy event CIDs remain valid and a new protocol version can adopt
new codecs. At scale, maintaining two implementations may become expensive.

### B: Keep Rust and bind to the Go Grid library

This maximizes implementation reuse without rewriting the domain and CLI.
Plausible boundaries include a subprocess protocol, a local Grid service, a C
ABI produced by Go, or a narrow generated FFI shim.

A subprocess or local service has the clearest crash and version boundary and
matches the Grid direction of independently upgradable storage agents. A Go C
archive has lower call overhead but complicates allocation ownership, error
types, cancellation, cross-compilation, panic containment, and debugging.

Concurrent actors benefit from a service boundary, but installation and
recovery now involve two binaries. A stable protocol is required before either
side can evolve safely. At scale, this route reuses Grid storage and sync while
retaining Rust domain checks.

### C: Rewrite the todo application in Go

Direct library use and deployment become straightforward. The current domain is
small enough that a rewrite is conceivable, but storage migrations, exact event
compatibility, CLI behavior, and all end-to-end tests must be preserved.

Go does not provide Rust-style exhaustive enum checking or newtype enforcement
by default. Typed constants, unexported constructors, sum-type interfaces,
total switch helpers, code generation, and strict static analysis can narrow
the gap, but they do not reproduce Rust's compiler guarantees completely.

Useful Go gates to investigate include `go vet`, `staticcheck`, `golangci-lint`
with an explicitly pinned configuration, `govulncheck`, race tests, fuzzing,
exhaustive-switch analyzers, and architecture/import-boundary tests. Tooling
availability and false-positive cost must be measured rather than assumed.

A rewrite has the highest migration risk: two implementations can temporarily
become competing authorities. It should be selected only if a vertical Go
prototype demonstrates materially lower total complexity than the service or
Rust-compatible routes.

### D: Go Grid sidecar with a language-neutral todo domain protocol

This is a constrained form of route B. Rust retains capture, lifecycle, focus,
and CLI policy. Go owns generic envelopes, CAS blocks, repository heads, and
network synchronization behind a narrow local protocol.

Normal single-machine todo use can continue when sharing is disabled. Failure
of the sidecar must not corrupt the local ledger, and pending publication must
be retryable. Concurrent and multi-user operation align naturally with the
Grid storage-agent model. Migration can proceed capability by capability.

## Current assessment

Do not rewrite yet. Route D is the most promising direct-reuse experiment, with
route A as the near-term baseline. The first bounded test should send exact
todo event envelopes through a Go Grid process and verify the same bytes, pCID,
event CID, visible links, unknown-pCID behavior, and failure recovery from Rust.

The experiment must compare total operational complexity, not only lines of
application code. No language or binding route is selected until that evidence
exists.


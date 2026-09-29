# TE-0014: Multi-User Access and Hosting

Date: 2026-09-29
Status: deferred pending PromiseGrid 1.0

## Decision Under Test

How should people who are not programmers reach this ledger?

Today the only access paths are a Rust CLI and a native egui window, both
reading a local SQLite projection, synchronizing through a Git checkout that
the operator configured by hand (TE-0007, DI-0007). Every one of those steps
assumes a developer. The decision is what to build so that a friend, or a
browser, can use the same ledger.

## Assumptions

- The event log is the authority; SQLite is a rebuildable projection (DI-0002).
- Synchronization semantics are already settled: the union of two event sets,
  deduplicated by CID, converges regardless of order (TE-0007 route D).
- `todo-core` has no platform dependencies. Its dependency set is chrono, cid,
  minicbor, multihash, serde, sha2, thiserror, and uuid, all of which build for
  `wasm32`. `todo-store` does not, because of turso, dirs, and the multithreaded
  tokio runtime.
- The graph editor reaches the store through 37 uniform call sites of the shape
  `runtime.block_on(self.ledger.<method>)`, so the store boundary is narrow.
- `eframe` supports `wasm32` and renders to a canvas.
- The ledger has no concept of a user. `actor_id` is a per-configuration UUID
  that tags events for provenance; it authorizes nothing and gates nothing.

## The Prior Question

Whether a shared ledger means one shared list or many private lists is not an
implementation detail of any route below. It decides whether events need an
owner and a visibility scope, which is an event-protocol change and therefore a
protocol version bump. It must be answered before a transport is built, because
every route below would otherwise be built against the wrong data model.

## Routes

### A: Ship The Native App Against A Shared Git Remote

Each person installs the binary; the operator runs `todo sync init` on their
machine once. A Sync button in the window can call the existing code path, so
Git never appears in the interface.

This needs no new infrastructure and no protocol change. Its cost is entirely
in onboarding: an unsigned macOS binary presents a Gatekeeper refusal, so
distribution implies Apple Developer enrollment and notarization, and every
participant needs a Git account plus an SSH key. The account and key are the
step that actually fails for a non-programmer; the rest is tolerable.

Appropriate for a handful of people the operator can physically set up.

### B: A Sync Server Replacing Git As The Transport

Because convergence is by CID union, the server is an append-only `(cid, bytes)`
table and two operations: submit envelopes, and read envelopes after a cursor.
Inserting a known CID is a no-op, so there is no merge logic, no conflict
resolution, and no ordering requirement on the wire.

This removes Git accounts, keys, and the `merge=union` driver from the user's
view, and replaces last-writer-wins file movers with an explicit protocol. It
adds a service to operate, an authentication decision, and a second place where
events live. Participants still install the application.

The interesting property is that this is the smallest change that makes the
system multi-party, and it is a prerequisite for route C rather than an
alternative to it, so it is not wasted work under any later choice.

### C: A Browser Build Of The Existing Editor

`todo-core` crosses to `wasm32` unchanged. The work is to put the store behind a
trait, keep the Turso implementation for native, and add a browser implementation
over IndexedDB that synchronizes against route B. Nothing is installed, nothing
is signed, and Gatekeeper never appears.

This is the only route that reaches someone who will not install software. It is
also the largest: a trait extraction across 37 call sites, a second store
implementation, a WebAssembly build pipeline, and hosting. It cannot be reached
without route B, or something equivalent to it, existing first.

### D: Wait For PromiseGrid 1.0

TE-0008 stages the product as one local ledger, then one ledger across several
repositories, then personal space, shared space, and multiple users. Multi-user
access is that third stage. The Grid work is expected to provide the shared CAS,
reference sets, and synchronization library that routes B and C would otherwise
implement locally, and TE-0008 has not yet selected a language or binding route.

Building a private sync server now means building the component most likely to
be replaced, against a protocol that has not been chosen.

## Scenario Analysis

### A Friend Wants To Add One Task

Under A they install a binary, clear a security dialog, create a Git account, and
generate a key. Under B they install a binary and paste a token. Under C they
open a link. The gap between A and C is the entire question.

### Two People Edit The Same Task While Offline

All routes converge, because all of them union events by CID. None of them
report that it happened: the projection resolves by `(recorded_at, event_id)`,
which is last-writer-wins per field with the fork preserved in `parent_cid`.
Surfacing divergence is a separate piece of work that no route provides and that
becomes considerably more visible with more than one person.

### The Grid Library Arrives

Routes B and C would each have to be re-pointed at Grid storage, and any
authentication or ownership model invented for them re-examined against the Grid
model. Route A is indifferent: it has no bespoke protocol to discard.

### The Operator Stops Wanting A Third-Party Remote

A moves to a self-hosted Git remote with no code change, only a different URL.
B and C already require self-hosting, so nothing changes.

## Narrowed Conclusions

- Route A is available now, costs nothing structural, and is sufficient for a
  small number of people who can be set up in person. Its ceiling is account and
  key management, not code.
- Route B is the smallest genuine unlock and cannot be skipped on the way to C.
- Route C is the only answer to "no installation" and is the natural endpoint,
  but every part of it except the browser store is downstream of B.
- Routes B and C both invent a synchronization protocol and an ownership model
  that TE-0008 expects the Grid to supply. Building them now risks producing the
  competing authority that TE-0008 warns against for a rewrite.
- The shared-versus-separate ledger question is upstream of all four routes and
  is unanswered.

## Decision

Deferred until PromiseGrid reaches 1.0, per route D. No sync server, browser
build, or store trait is introduced now.

Route A remains available in the meantime and requires no decision recorded here;
if it is taken, it should be taken as distribution work, not as architecture.

Two items are to be settled before the deferral lifts, because neither depends
on the Grid and both would otherwise be decided by accident:

- whether a shared ledger is one list or many, with the event-protocol
  consequences of an owner and a visibility scope;
- whether concurrent divergence is reported to a person or resolved silently, which
  is already true today and only becomes visible with more than one writer.

Recorded by agent-assistant.

# Repository Guidelines

## Product Direction

- Build a global, local-first todo ledger. Projects are logical scopes inside
  the global ledger, not separate databases.
- Keep exact immutable events separate from rebuildable relational views.
- Keep Promise Theory compatibility in mind without forcing promise vocabulary
  into ordinary capture and completion workflows.
- Implement in this order: decision artifacts, domain API and tests, storage
  and tests, CLI and end-to-end tests, then dogfood handoff.

## Decision Workflow

- Use a thought experiment before locking a durable design when several
  plausible alternatives remain. Compare normal use, failure, concurrent
  actors, migration, and scale.
- Record unresolved blockers in `DR/` and settled decisions in `DI/`.
- Preserve decision history. Supersede prior decisions; do not silently rewrite
  their meaning.
- A behavior-changing implementation must cite its governing DI in code or its
  test when the intent is not obvious.

## Implementation

- Keep the domain API independent of the CLI and Turso.
- Store exact Grid-CBOR event bytes before deriving query views.
- Keep Turso behind a narrow store boundary and avoid experimental SQL features
  until a TE and DI justify them.
- Use explicit enums and typed identifiers instead of boolean state or loosely
  interpreted strings in domain code.
- Keep the CLI opinionated for normal use while providing flags for wider views.

## Verification

- Run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  and `cargo test --workspace` before handoff.
- Every CLI behavior needs an end-to-end test using isolated config and data
  directories.


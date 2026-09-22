---
name: rust-development
description: Develop or review Rust changes with repository-native checks, typed boundaries, and focused tests. Use when changing Rust code, Cargo configuration, or workspace structure.
---
# Develop Rust Changes

## Start With Repository Practice

Read the relevant crate, workspace, documentation, and existing tests. Use a
documented `just`, `make`, or other repository wrapper when it encodes project
policy. Use stable Rust unless the repository explicitly pins another toolchain.

For a strict workspace without a stronger repository command, run:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

During iteration, run focused crate or behavior tests. Run the full workspace
checks before finalizing a cross-crate, dependency, interface, or persistence
change.

## Keep The Boundary Typed And Narrow

Use typed structures and enums for domain concepts rather than distributing
strings through command handling. Prefer enums, named methods, newtypes, or a
builder when they make intent clearer than boolean or ambiguous `Option`
parameters.

Before a first implementation slice that constrains later work, identify its
responsibility and public compatibility boundary. Keep parsing and domain
operations behind typed interfaces so CLI, GUI, or service adapters can share
behavior without prematurely requiring an async or multi-crate design.

## Control Dependencies And Generated Changes

Do not add a database, async runtime, GUI framework, or broad dependency unless
the requested behavior needs it. When dependency files change, run the
repository's documented lockfile refresh command and include resulting updates.

Do not add optional Rust tools, a nightly toolchain, or a strict lint profile
without a repository decision. Add development tools through the repository's
declared environment rather than ad hoc installation.

## Keep Production Paths Strict

For a long-lived Rust project, adopt this Cargo lint profile unless the
repository has an approved alternative:

```toml
[lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
todo = "deny"
unimplemented = "deny"
unreachable = "deny"
indexing_slicing = "deny"
as_conversions = "deny"
```

Production code must not use unchecked panics, placeholders, indexing, or casts
that this profile rejects. Use a narrow local `#[allow(..., reason = "...")]`
only when the invariant is established and the repository permits it. Test code
may use separately approved exceptions for readable fixtures and assertions.

Do not enable the whole `clippy::pedantic`, `clippy::nursery`, or
`clippy::restriction` groups. Run their lints as warnings first, then promote
only proven useful lints to the project profile.

## Verify

Keep tests deterministic and prefer whole-object equality when it makes failures
clear. Report the focused checks run, broader checks intentionally omitted, and
any compatibility or dependency decision that still needs developer approval.

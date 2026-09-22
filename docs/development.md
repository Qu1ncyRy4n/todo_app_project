# Development Tooling And Quality Policy

## Current baseline

The repository uses the pinned Nix flake as its Rust toolchain. Run checks
through the project-local `./dev` wrapper, which works without a globally
installed Rust toolchain:

```sh
./dev fmt
./dev check
./dev test
```

`./dev check` runs `cargo fmt --check` and strict Clippy across the workspace,
all targets, and all features. `./dev test` runs the full workspace test suite.
Use focused crate tests while iterating, then run both commands before completing
cross-crate, CLI, protocol, or persistence work.

The enforced Clippy restrictions prevent compiled placeholder and panic paths,
unreachable macros, and unchecked `as` conversions:

```toml
[workspace.lints.clippy]
panic = "deny"
todo = "deny"
unimplemented = "deny"
unreachable = "deny"
as_conversions = "deny"
```

Each crate inherits the workspace lint profile. Checked `TryFrom` conversions
are preferred where a numeric value crosses a type boundary.

## Deferred improvements

The following are deliberate evaluation items, not current dependencies or
requirements:

- Test execution: evaluate `cargo-nextest` only if the suite becomes slow or
  needs retries, reporting, or test partitioning.
- Test confidence: add property tests and failure-injection tests when a
  concrete invariant is at risk, especially for event encoding, replay,
  transactional imports, and Git synchronization convergence.
- Error safety: audit production uses of `unwrap`, `expect`, and direct
  indexing before enabling `clippy::unwrap_used`, `clippy::expect_used`, or
  `clippy::indexing_slicing`. Replace fallible paths with typed errors; retain a
  narrow, documented exception only where an invariant is established locally.
  Test fixtures may need separately scoped exceptions for readable assertions.
- Supply-chain and dependency checks: evaluate `cargo-deny`, `cargo-audit`, and
  unused-dependency tooling when CI or release policy introduces that need.
- Developer workflow tools: add `direnv`, Devenv, watch tools, benchmarks, or
  mutation testing only for demonstrated needs. The existing direct Nix flake
  remains the default toolchain definition.

Do not enable broad Clippy groups such as `pedantic`, `nursery`, or
`restriction` wholesale. Trial individual lints as warnings, fix or document
their concrete findings, then promote only those that improve this repository.

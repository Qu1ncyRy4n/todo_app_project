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

`./dev gui` runs a release-optimized editor by default; use
`./dev gui --debug` when debugging, or `./dev gui --release` explicitly.
`./dev build` retains its debug default, and accepts `--debug` or `--release`
for a whole-workspace build. The first release build compiles a separate Cargo
profile and can take longer than later launches.

`./dev check` runs `cargo fmt --check` and strict Clippy across the workspace,
all targets, and all features. `./dev test` runs the full workspace test suite.
Use focused crate tests while iterating, then run both commands before completing
cross-crate, CLI, protocol, or persistence work.

The enforced Clippy restrictions prevent compiled placeholder and panic paths,
unchecked `unwrap`/`expect`, unreachable macros, unchecked indexing or slicing,
and unchecked `as` conversions:

```toml
[workspace.lints.clippy]
panic = "deny"
todo = "deny"
unimplemented = "deny"
unreachable = "deny"
as_conversions = "deny"
unwrap_used = "deny"
expect_used = "deny"
indexing_slicing = "deny"
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
- Error safety: retain narrow, documented exceptions only where an invariant is
  established locally. Test fixtures have separately scoped exceptions for
  readable setup and assertions. Revisit those exceptions when test helpers or
  assertion APIs make the intent clearer.
- Supply-chain and dependency checks: evaluate `cargo-deny`, `cargo-audit`, and
  unused-dependency tooling when CI or release policy introduces that need.
- Developer workflow tools: add `direnv`, Devenv, watch tools, benchmarks, or
  mutation testing only for demonstrated needs. The existing direct Nix flake
  remains the default toolchain definition.
- Shell completion: zsh binds its bundled `_devtodo` completer to `todo`, so
  Tab runs this binary with devtodo's flags and fails with
  `unexpected argument '-f' found`. Planned fix: add `clap_complete`, expose
  `todo completions <shell>`, write the zsh script to a directory on `fpath`
  such as `~/.zfunc/_todo`, and run `compdef _todo todo` after `compinit` so it
  overrides `_devtodo`. Until then, `compdef -d todo` in `.zshrc` silences the
  error. Item IDs can be completed dynamically later.

Do not enable broad Clippy groups such as `pedantic`, `nursery`, or
`restriction` wholesale. Trial individual lints as warnings, fix or document
their concrete findings, then promote only those that improve this repository.

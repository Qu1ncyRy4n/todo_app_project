# Project Handoff

Updated: 2026-10-14
Covers: workspace v0.3.0, protocol v13, `main` at "docs: update pickup notes
after the October 14 session".

## Where we left off, October 14

- **Shipped:** dark mode with a Ghostty-format palette, and the editor no longer
  locks the CLI out. Details are in the graph editor handoff, slices 54 to 56.
- **Design direction agreed, not built:** TE-0016 (one-line typed entry,
  actors, roles on links, projects keyed by repo, KDL manifests, `relates-to`)
  and TE-0017 (habits as occurrence events summarized by stats). The
  quick-entry research is `docs/research/task-field-survey.md`.
- **Ledger:** todo-app work lives under `ruris-nipal` (feature sprint) and
  `tafuf-hohob` (identity and sync sprint), tagged `tag/work/apps/todo-app`,
  with GUI items under `…/gui`. This repo is not attached as a project; tags
  carry it, pending TE-0016's repo-keyed projects.
- **Queued quick wins:** document the current appearance defaults
  (`kapah-danip`); `todo import markdown --parent <id>` with `--dry-run`
  (`rutas-mumik`; show usage and output before building); an agent skill for the
  todo CLI (`zitun-funiv`).
- **Deferred:** `clap_complete` shell completion (`docs/development.md`). zsh
  binds its `_devtodo` completer to `todo`; `compdef -d todo` silences it.
- **External:** another agent is wiring matugen through Nix to generate the
  Ghostty theme `[colors] palette_file` points at.

This is the entry point for picking the project up cold. It states what exists,
what is deliberately not built, what is unverified, and where each area's own
handoff lives.

## Orientation in five minutes

```sh
nix develop          # everything runs in the pinned shell; ./dev wraps it
./dev check          # cargo fmt --check, then clippy -D warnings
./dev test           # 236 tests
./dev gui            # the graph editor against the real ledger
./dev run -- ls --tree
```

Read in this order: this file, then `docs/architecture.md` for the model, then
the area handoff you need. Decisions are in `DI/`, open questions in `DR/`, and
reasoning-before-deciding in `docs/thought-experiments/`. A TE that has been
decided says so in its `Status` line.

## What the system is

One global event-sourced ledger. Events are immutable Grid-CBOR envelopes
addressed by CID; SQLite is a projection that can be thrown away and rebuilt.
Two clients read it: the `todo` CLI, which is the complete surface, and
`todo-gui`, a native graph editor that is presentation only. Both can run at
once: the editor opens the ledger per operation, because Turso locks the file
for as long as it is open.

```text
crates/todo-core    domain, events, CIDs, proquints. No platform dependencies.
crates/todo-store   Turso projection and configuration.
crates/todo-cli     the complete control surface.
crates/todo-gui     eframe graph editor over todo-store.
```

The invariant worth protecting: `todo-core` depends on nothing platform-specific
and builds for `wasm32`. Everything that would ever run in a browser is
downstream of that staying true.

## Area handoffs

- **Graph editor** — `docs/gui-graph-pickup.md`. Slices 1 to 56, every
  configuration knob, the eight items of remaining work, and what is deferred by
  decision.
- **Family display and Focus** — `docs/thought-experiments/TE-0015-graph-display-flow.md`.
  An approved trial, not a settled appearance: three questions stay open pending
  a running-window review.
  This is the most active area and the most detailed handoff.
- **Multi-user and hosting** — `docs/thought-experiments/TE-0014-multi-user-access-and-hosting.md`.
  Deferred pending PromiseGrid 1.0. Four routes analyzed, two prerequisite
  questions named.
- **CLI usability** — `docs/cli-ux-audit-2026-09-04.md`. A hands-on review with
  prioritized gaps; older than the rest of this document and worth re-running
  before trusting its priorities.
- **PromiseGrid and language** — `docs/thought-experiments/TE-0008-...md` and
  `DR-0002`. Still open: no language or binding route is selected.

## Synchronization, as actually configured

Working and in use, not merely implemented.

```text
~/Library/Application Support/todo-ledger/   config.toml, theme.toml,
                                             gui-layout.json, ledger.db (+ WAL)
<sync repo>/                                 events.jsonl, ledger.toml,
                                             .gitattributes
```

`config.toml` carries `sync_repo`, pointing at a Git checkout whose
`.gitattributes` sets `events.jsonl merge=union`. `todo sync` pulls, imports
unknown CIDs, rewrites the log canonically, commits, and pushes, retrying once
through a union merge if a peer pushed first. The database never syncs.

Setting up an additional machine is **clone first, then init**, because
`todo sync init` on an empty directory creates a fresh repository with no
remote:

```sh
git clone <remote> <path> && todo sync init --repo <path> && todo sync
```

Two properties to keep in mind rather than rediscover:

- Convergence is automatic and conflict-free for `events.jsonl`. Every other
  file in the sync repository, `README.md` included, merges normally and can
  conflict.
- Convergence is not agreement. Concurrent edits to the same field both survive
  as events; the projection picks by `(recorded_at, event_id)`. Nothing reports
  that a divergence occurred. This is already true with one person and only
  becomes visible with more.

## Protocol discipline

New writes use v13. Import accepts v5 through v13. Protocol documents in
`docs/protocols/` are content-addressed: their bytes are hashed into every event
that cites them, so **a protocol file is never edited in place**. A new field
means a new version file and a new CID.

Anything stored, exchanged, or replayed is not configurable. Anything about
appearance or arrangement is. That line is the reason `theme.toml` can be
rearranged freely while the event payloads cannot.

## What is unverified

Stated plainly so it is not mistaken for tested ground.

1. **The editor has no window-level tests.** Coverage is the pure layers only:
   `geometry.rs`, `theme.rs`, `palette.rs`, `tags.rs`, and free functions lifted
   out of `main.rs` such as `matches_query`, `limit_child_depth`,
   `bounded_family_box`, and `focus_is_active`. Panel wiring, hotkeys, and focus
   handling are confirmed by running `./dev gui` and nothing else. Nothing in the
   TE-0015 work has been seen in a running window: not the `/` or `G` hotkeys,
   not the parent-to-board stalk, not the continuation chip, and not the Focus
   separation that was the original bug. The editor handoff lists what to check
   and in what order.
2. **Multi-machine sync has been exercised in one direction only**, from the
   machine that first ran `todo sync init`. The clone-then-init path on a second
   machine is correct by inspection of `init_sync_repo` and `sync`, not by
   having been run.
3. **Concurrent-write convergence is argued, not demonstrated.** The reasoning is
   sound because CID union is order-independent, and the CLI suite covers import
   deduplication, but two machines appending while offline has not been staged as
   a test.
4. **`todo-core` building for `wasm32` is inferred from its dependency set**,
   which contains nothing platform-specific. No `wasm32` target has been added to
   the build or attempted.

## Deliberate absences

Not oversights. Each has a reason recorded.

- **Deletion in the editor.** Removed at the developer's request after being
  built. `todo rm --force` and `todo restore` remain; protocol v10 still carries
  the events. A forced purge behind a retype-the-title confirmation is deferred.
- **A scheduled time of day.** The ledger stores a date only, so the timeline has
  no hourly dimension to place work against. Needs a field and a protocol
  version; it is the next thing the timeline wants.
- **Users and permissions.** `actor_id` tags events for provenance and authorizes
  nothing. Whether a shared ledger is one list or many is unanswered and is an
  event-protocol question, not a transport one. See TE-0014.
- **Divergence reporting.** See above; no route in TE-0014 supplies it.
- **Live physics, timeline snapping, CLI grouping and sorting.** Listed under
  "Deferred by decision" in the graph editor handoff.

## Conventions a new agent will otherwise get wrong

- Build only through `nix develop` or `./dev`. A bare `cargo` picks up rustc
  1.92 and fails on `eframe`, which needs 1.95.
- Workspace clippy denies `panic`, `todo`, `unimplemented`, `unreachable`,
  `as_conversions`, `unwrap_used`, `expect_used`, and `indexing_slicing`. Test
  modules need an explicit `#[allow(..., reason = "...")]`.
- Read clippy output rather than counting its lines. Commits have twice gone in
  with lint errors because a `grep -c` returned a number instead of the errors.
- Commits carry no agent trailers or signatures. Decision records attribute to
  `agent-assistant`.
- Default commit mode is commit-only-when-asked. Atomic commits require an
  explicit goal.
- Deliberate shortcuts are marked with a `ponytail:` comment naming the ceiling
  and the upgrade path. There are four, all in `crates/todo-gui/src/geometry.rs`.
  The broader list of known-imperfect behavior is "Remaining work" in the graph
  editor handoff, which is seven items and not the same list.

Recorded by agent-assistant.

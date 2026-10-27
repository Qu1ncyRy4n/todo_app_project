# Project Handoff

Updated: 2026-10-26
Covers: workspace v0.3.0, protocol v15, graph verification through `82c4ef8`.

## Where we left off, October 26

- **Empty-canvas deselection (`41b4e48`).** A plain click off cards now clears
  both the Shift-selection and the Info-focused card. A synthetic egui pointer
  test exercises the click path in addition to the state-level test. The editor
  builds and launches here, but the standalone window is not exposed to this
  environment's window-control interface, so visual acceptance remains with
  the developer.
- **Graph normalization (`168c66c`, `f3a0dcd`).** Canvas G, card/family G,
  and Shift-selected G now compute positions through the same configured
  Hierarchy or Layered placement. Family G reveals hidden descendants, anchors
  its root, and retains other families' drawn positions. Selected G changes
  only selected cards, anchored around their previous center. Canvas G still
  frames the viewport and may run graph-wide separation and overlap relaxation;
  local G does not move unrelated cards through those post-passes. Automated
  tests cover both styles, nested reveal, unaffected positions, and Undo.
- **Recent graph controls (`4d7b42b`, `d99f9da`, `1744b56`).** Family badges
  report their current `+`, `-`, or `-*` visibility. C then quick CC contracts
  inactive then every descendant in the same scope. Shift-click and Shift-drag
  make a temporary selection; C, E, and G operate on it. The developer confirmed
  these controls and shared G normalization in the running app on October 26;
  `pizof-tohod`, `kajim-sifog`, `zadin-nizud`, `mumuf-nukum`, `vohas-bitop`, and
  `hurik-duliv` are done in the CLI ledger.
- **Still open:** The exact empty-canvas deselect gesture (`rugub-tivit`) has
  synthetic pointer coverage but awaits developer review. Relation-based
  proximity/order is split into `voros-vomoh`: symmetric `relates-to` cannot
  be treated as directed precedence. The done timeline (`punun-hopoj`) has not
  been built.
- **Validation for the normalization tree:** `./dev fmt`, `./dev check`,
  `./dev test`, and `./dev build --release` passed. The later deselection test
  commit also passed `./dev check` and `./dev test` (185 GUI tests). No push or
  sync was performed.

## Where we left off, October 20

- **Agent signatures (protocol v15, `6678f52`).** Events may carry
  `agent = "<name>"`. `TODO_AGENT` names the agent (`TODO_AGENT=` is the
  operator); otherwise `agent_env` variables (default `CLAUDECODE`,
  `GEMINI_CLI`, `CODEX_SANDBOX`) sign as `agent`. Agent-created items show
  `[agent]`; `ls`/`ready`/`next --agent show|hide|only`, default from
  `agent_items`. `todo whoami` reports the signature. Item creation now stamps
  the device `actor_id`, which `add` and import used to leave as `local`.
  Not built: linking the device `actor_id` to a person actor, completion
  credit (both in `nafun-rinan`, deferred to 2026-11-01).
- **Existing items labeled (2026-10-20):** the 19 items agent-assistant
  created (found in this project's agent transcripts) carry
  `by:agent-assistant`, an actor with `kind = "agent"`; the other 122 carry
  `by:quincy`. An author whose kind is agent counts as an agent item for
  `--agent`, which is how pre-v15 items are filtered. The developer confirmed
  no other agent has added items, so the labels are complete.
- **Before syncing v15 events,** every machine needs `./dev install`. The real
  ledger now holds v15 events.
- **Manifest:** `members = [...]` on orgs as well as `member-of` on members
  (`2d21e8c`); `based_on` now hashes records, not layout (`bc135c8`). Whether
  the manifest is worth keeping is an open question in TE-0016.
- **GUI:** widget visuals follow the palette background (`49721f2`), fixing
  dark text on dark dropdowns with a light palette. Not yet seen in a window.
- **GUI layout (TE-0018, `9de2b72`..`31c7fe2`).** Filter drawer (`F`),
  floating Settings window (`,`), read-first inspector (`I`) with colored
  People, Time, Structure, Context, and Notes bands and in-place editing; a
  filter chip row above the canvas; `[colors.inspector]` theme keys. Pickup
  slices 59-60 list what to try. Not yet seen in a window. Removing a link
  needs a ledger call that does not exist yet.

## Earlier, October 19

- **Identity sprint (`tafuf-hohob`) built**, per TE-0016:
  - Protocol v14: fields on namespace nodes, `member-of` links, roles on
    assignments, symmetric `relates-to` (`93c4ac6`).
  - Typed entry in `todo add`, plus `todo set` and `todo actor` (`ce7d4ac`). The
    parser is `todo_core::entry`, ready for the GUI quick-entry bar
    (`popal-nofah`). `config.toml` takes `me = "<name>"` for `by:me` and
    `entry_ascii = false` to turn off `#`, `@`, `+`.
  - Projects keyed by repo (`382342d`): `todo project repo <name> [url]`; a
    checkout whose `origin` matches infers the project on any machine.
  - `todo scan [PATH...] [--check]` (`9fccad0`) lists `todo:<id>` references
    with each item's state.
  - `todo manifest export|apply` (`cb678af`): actor and project records as
    TOML, with a `based_on` digest that refuses stale files. TOML is a
    stopgap; Mogent's small language (`~/dev/cdint/Agents`) is the likely
    long-term syntax once it settles. Reasoning and ideas kept: TE-0016.
- **Deferred to 2026-11-01:** sync redesign (`vuzil-gofah`), sync attribution
  (`nafun-rinan`), mobile stack (`kibut-hupan`).
- **Not built from TE-0016:** emoji entry forms, `!1` priority (zsh history
  expansion eats it), project default tags. Multi-word `add`/`set` writes one
  event per word, not one transaction.
- **Before syncing v14 events:** every machine needs the new binary
  (`./dev install`, now a Nix profile package), or import fails with "uses an
  unknown protocol".
- **Ledger:** todo-app work lives under the roadmap `ruris-nipal`, split into
  sprints: identity and sync (`tafuf-hohob`), GUI (`kufod-mogar`, with a
  needs-spec views sub-sprint `tibum-zivim`), entry and CLI ergonomics
  (`tasir-visok`), model and relations (`tajam-havag`), naming and config
  (`razam-nobir`), and modes and navigation (`gusol-difon`). Tagged
  `tag/work/apps/todo-app`, GUI items under `…/gui`. This repo can now be keyed
  with `todo project repo`; the developer has not chosen to yet.
- **Modes (needs spec):** `nogul-darat` holds the developer's sketch of a DO
  sequence (a stack, one open item, no wandering) and a PLAN traversal (fast
  task/subtask/dependency navigation). It depends on the binding layer
  `viguz-nodit`.
- **Skill:** `todo-ledger` (`zitun-funiv`, done) is committed in
  `qmr-agents-library` and copied into `.agents/skills/` by hand, because
  `mogent build` fails on a front-matter error in that library's archive.
- **Deferred by the developer:** the rename (`jalil-kimop`, to 2026-10-22);
  `todo import markdown --parent <id>` (`rutas-mumik`; show usage and
  `--dry-run` output before building).
- **Deferred:** `clap_complete` shell completion (`docs/development.md`). zsh
  binds its `_devtodo` completer to `todo`; `compdef -d todo` silences it.
- **External:** another agent is wiring matugen through Nix to generate the
  Ghostty theme `[colors] palette_file` points at. No editor-side hook is
  needed: theme watch picks up regenerated files and re-pointed symlinks.

This is the entry point for picking the project up cold. It states what exists,
what is deliberately not built, what is unverified, and where each area's own
handoff lives.

## Orientation in five minutes

```sh
nix develop          # everything runs in the pinned shell; ./dev wraps it
./dev check          # cargo fmt --check, then clippy -D warnings
./dev test           # 253 tests
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

- **Graph editor** — `docs/gui-graph-pickup.md`. Slices 1 to 57, every
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

New writes use v15. Import accepts v5 through v15. Protocol documents in
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
  and the upgrade path. `grep -rn "ponytail:" crates` lists them (nine today).
  The broader list of known-imperfect behavior is "Remaining work" in the graph
  editor handoff, which is seven items and not the same list.

Recorded by agent-assistant.

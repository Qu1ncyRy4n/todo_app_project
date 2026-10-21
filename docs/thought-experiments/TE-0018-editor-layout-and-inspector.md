# TE-0018: Editor Layout and a Calmer Inspector

Date: 2026-10-20
Status: implemented 2026-10-20 with every proposed choice (see Outcome);
awaiting a check in a running window.
Area: `crates/todo-gui`, handoff `docs/gui-graph-pickup.md`

## Decision Under Test

How can the editor stop feeling dense, so that the graph is the main thing
on screen, filters are one click away, and the inspector reads at a glance and
only shows controls when you ask for them?

## What is on screen today

```text
┌ toolbar ─────────────────────────────────────────────────────────────────────┐
│ Todo Ledger [New task] | − + [Reset view] | [Reload theme] [Refresh] |       │
│ [Filters] [Inspector] | Search [________]                                    │
├ left: "filters" ──────┬ graph ─────────────────────┬ right: inspector ───────┤
│ [Reset filters]       │                            │ Inspector               │
│ ▾ View                │                            │ inbox · tafuf-hohob     │
│   hierarchy, timeline │                            │ ▾ Task basics           │
│   group by ×3, order  │                            │   Title [____] [Save]   │
│ ▸ Appearance  (~200   │                            │   Status [▾] [Save]     │
│   lines of knobs)     │                            │   Priority [▾] [Save]   │
│ ▾ Arrangements        │                            │ ▾ People                │
│ ▸ Forces              │                            │ ▸ Family & dependencies │
│ ── Filters ──         │                            │ ▸ Tags, notes & links   │
│ ▾ Tasks shown         │                            │ ▸ Schedule & effort     │
│ ▾ Relationships drawn │                            │ ▸ Advanced / raw JSON   │
│ ▾ Overlays            │                            │ ▸ Visible fields        │
│ ▾ Tags                │                            │                         │
└───────────────────────┴────────────────────────────┴─────────────────────────┘
```

Problems, in order of how much they cost:

1. **The left panel mixes four jobs.** How the graph is arranged, how it looks,
   how the simulation moves, and what is filtered all share one scrolling
   column. Filters, the most used part, sit at the bottom.
2. **Nothing says what is hidden while the panel is closed.** Closing the
   filter panel hides the state as well as the controls.
3. **The inspector is a form, not a summary.** Every field shows an editor and
   a Save button, empty or not. Reading what a task *is* means scanning
   inputs.
4. **The sections look the same.** Nothing tells people apart from dates
   apart from structure, so the eye has no anchor.
5. **Rarely used things sit beside frequently used ones.** Reload theme,
   Refresh (already automatic), raw JSON, and the Visible fields list take
   space every session.

## Proposal

Three surfaces, each with one job:

| Surface | Job | Opens with |
|---|---|---|
| **Filter drawer** (left) | what is shown | `Filter` button, `F` key |
| **Settings window** | how it looks and moves | `⚙` button, `,` key |
| **Inspector** (right) | what the selected task is, and editing it | selecting a card, `I` |

Plus one thin **filter bar** above the canvas that is always visible when any
filter is active.

### Sketch 1: the default window

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ [+ New]  [Filter · 3]  [Layout ▾]   − 100% +  ⌖   [/ search_______]  ⚙  [▣] │
├──────────────────────────────────────────────────────────┬───────────────────┤
│ hiding: done ✕   agent tasks ✕   tag: life ✕   clear all │  INSPECTOR        │
│                                                          │  (Sketch 4)       │
│                                                          │                   │
│                    graph, edge to edge                   │                   │
│                                                          │                   │
│                                                          │                   │
└──────────────────────────────────────────────────────────┴───────────────────┘
```

- **Toolbar, actions only.** `Filter · 3` shows how many filters are active.
  `Layout ▾` holds the arrangement actions (Normalize, Focus layout, Group,
  Undo layout). `⌖` is Reset view. `⚙` opens Settings. `▣` toggles the
  inspector. Reload theme and Refresh leave the toolbar: the editor already
  reloads both on its own, and Settings keeps a Reload button for the odd case.
- **Filter bar.** Only appears when a filter is active. Each chip removes its
  filter on click; `clear all` is Reset filters. This answers "why is my task
  missing?" without opening anything.

### Sketch 2: the filter drawer open

```text
┌───────────────────────┬──────────────────────────────────┬───────────────────┐
│ FILTER          reset │ hiding: done ✕  agent tasks ✕    │                   │
│ ───────────────────── │                                  │                   │
│ Search  [__________]  │                                  │                   │
│                       │                                  │                   │
│ Tasks                 │                                  │                   │
│  ☐ done & canceled    │                                  │                   │
│  ☐ active work only   │                                  │                   │
│  Agent-created [Hide▾]│                                  │                   │
│                       │                                  │                   │
│ Tags                  │                                  │                   │
│  ◐ work          12   │                                  │                   │
│    ● cdint        5   │                                  │                   │
│    ○ exocortex    4   │                                  │                   │
│  ○ life           9   │                                  │                   │
│                       │                                  │                   │
│ Lines drawn           │                                  │                   │
│  ☑ parents ☑ depends  │                                  │                   │
│  ☑ blocked ☑ related  │                                  │                   │
│                       │                                  │                   │
│ Overlays              │                                  │                   │
│  ☑ tag regions        │                                  │                   │
│  ☑ parent regions     │                                  │                   │
│  ☑ focus window       │                                  │                   │
│  ☑ timeline ruler     │                                  │                   │
└───────────────────────┴──────────────────────────────────┴───────────────────┘
```

- Filtering only. View, Appearance, Arrangements, and Forces leave this panel.
- Lines drawn and Overlays stay here because they decide what is visible.
  They are compact two-column checkboxes, closed by default if space is tight.
- Tag rows show counts, so a tri-state click is informed.

### Sketch 3: the Settings window

```text
┌ Settings ──────────────────────────────────────────────────────── ✕ ┐
│ [Layout] [Cards] [Families] [Tags] [Motion] [Colors] [Inspector]      │
│ ───────────────────────────────────────────────────────────────────── │
│ Layout                                                                │
│   Hierarchy grows      [Down ▾]                                       │
│   Timeline runs        [Right ▾]                                      │
│   Group by, in order   [Hierarchy ▾] [— ▾] [— ▾]                      │
│   Order within group   [Priority ▾]                                   │
│                                                                       │
│                                                                       │
│ ───────────────────────────────────────────────────────────────────── │
│ theme.toml · unsaved changes      [Revert]  [Save to theme.toml]      │
│                                   [Reload from disk]                  │
└───────────────────────────────────────────────────────────────────────┘
```

- A floating, movable window, not a third side panel, so the graph keeps its
  width. Tabs replace the long Appearance scroll: Cards, Families, Tags
  (clouds and coloring), Motion (forces and spacing), Colors (mode and
  palette), Inspector (which sections and fields show, today's "Visible
  fields").
- The footer holds the theme-file actions that exist today and are scattered:
  save, revert, reload.
- Changes still apply live, as now.

### Sketch 4: the inspector, reading

```text
┌ INSPECTOR ──────────────────────────────┐
│ ▌ Todo app identity & sync sprint   ✎   │  title, click to edit
│ ▌ [inbox ▾] [normal ▾]  tafuf-hohob ⧉   │  state and priority chips
│ ▌ in  Todo app roadmap                  │  parent as a link
│                                         │
│ ▌PEOPLE                            + ▾  │  violet stripe
│ ▌ author     agent-assistant  ·agent    │
│ ▌ assignee   quincy                     │
│                                         │
│ ▌TIME                              + ▾  │  amber stripe
│ ▌ due        Nov 1  (in 12 days)        │
│ ▌ estimate   2 h                        │
│                                         │
│ ▌STRUCTURE                         + ▾  │  blue stripe, same blue as
│ ▌ children   8, 3 open                  │  family connectors
│ ▌ depends on vuzil-gofah redesign sync  │
│                                         │
│ ▌CONTEXT                           + ▾  │  green stripe
│ ▌ tags       work/apps/todo-app         │
│ ▌ links      TE-0016                    │
│                                         │
│ ▌NOTES  2                          + ▾  │  neutral
│ ▌ Oct 15  deferred with the sync model… │
│ ▌ show all                              │
│                                         │
│   history · raw · id 01a13cde…          │  quiet footer
└─────────────────────────────────────────┘
```

Rules that make this calmer:

- **Read first, edit on demand.** Values show as text. Click a value to edit
  it in place; Enter commits, Escape cancels. No Save buttons: each change is
  one event, as now.
- **Empty fields are not shown.** A task without a due date has no due row.
  `+ ▾` on each band lists what can be added there (Time: due, defer,
  scheduled, estimate, difficulty). An empty band shows only its header,
  dimmed.
- **One color per band**, as a thin left stripe and the band label, never as
  a fill behind text. The colors are theme tokens (below), so they follow
  light and dark palettes.
- **State and priority are chips** that open a menu on click. The state chip
  uses the card's status color, so the inspector and the card agree.
- **Rare things go to a footer:** event history, raw JSON, the full ID.
  "Advanced / raw editing" stops being a section.
- **Bands stay collapsible,** and collapsed bands show a one-line summary
  ("PEOPLE  author agent-assistant · assignee quincy").

### Sketch 5: editing inline (People)

The developer's sketch, `people [edit] / author: quincy / add [author]…`, is
nearly what exists. The change is that the add row appears only when asked
for, and rows edit in place.

```text
reading                              after `+ ▾` → assignee
┌───────────────────────────────┐    ┌───────────────────────────────┐
│ ▌PEOPLE                  + ▾  │    │ ▌PEOPLE                  + ▾  │
│ ▌ author   quincy             │    │ ▌ author   quincy             │
│                               │    │ ▌ assignee [ste_________]     │
│ hover a row:                  │    │ ▌          steve   stephanie  │
│ ▌ author   quincy        ✎ ✕  │    │ ▌          Enter adds · Esc   │
└───────────────────────────────┘    └───────────────────────────────┘
```

The same three moves apply to every band: hover a row for `✎ ✕`, click a
value to edit it, `+ ▾` to add a row of a chosen kind.

### Color tokens

New `[colors.inspector]` keys in `theme.toml`, with defaults chosen to sit
apart from the status colors and the tag hues:

| Band | Default | Why |
|---|---|---|
| people | violet | not used elsewhere |
| time | amber | reads as "deadline" |
| structure | `FAMILY_BLUE` | matches the family connectors on the canvas |
| context | green | tag regions already lean green in the default palette |
| notes | neutral grey | text-heavy; color would be noise |

A palette (`palette_file`) can override them. Only stripes and labels are
colored, so contrast with body text never depends on them.

## What this does not change

- No event, protocol, or store change. Pure presentation, so it is configurable
  (`theme.toml`), consistent with the handoff's rule.
- The CLI is unaffected.
- Graph drawing is unaffected, apart from the filter bar taking one row of
  height when shown.

## Staging

Each slice ships alone and is checked in a running window before the next.

1. **Settings window.** Move View, Appearance, Forces, and Visible fields into
   tabs; move theme-file buttons into its footer. The left panel becomes
   filters only. Smallest diff with the biggest calm.
2. **Toolbar and filter bar.** `Filter · n`, `Layout ▾`, `⚙`; drop Reload
   theme and Refresh from the toolbar; add the chip bar.
3. **Inspector header and footer.** Title, state and priority chips, parent
   link; history, raw, and ID to the footer.
4. **Inspector bands.** Read-first rows, `+ ▾` add menus, hidden empty
   fields, colored stripes, collapsed summaries. One band per commit, People
   first since it is the newest.

## Outcome

The developer approved the proposal as sketched, so each open choice took
the proposed option:

1. Settings is a floating window.
2. Empty fields are hidden; an empty band shows its header dimmed.
3. Band colors are the defaults above, set in `[colors.inspector]`; deriving
   them from the palette's ANSI colors stays open.
4. `F`, `,`, and `I` toggle the drawer, Settings, and the inspector, only while
   no text field has focus. They do not clash with `/` or `G`; `viguz-nodit`
   should absorb them when the binding layer lands.
5. Lines drawn and Overlays stay in the filter drawer, as two-column grids.

Landed as seven commits: the Settings window; toolbar, chips, and keys; the
inspector header and footer; then the People, Time, Structure, and
Context-with-Notes bands. Where the build differs from the sketches:

- The drawer has no search box of its own; the toolbar's search is the one
  search, and an active search shows as a chip.
- Chips name what differs from a fresh window ("showing done", "tag work
  only", "agent tasks hidden") rather than a fixed "hiding:" list. Agent
  filtering counts whenever it hides anything, even as the configured
  default. Reset filters now returns the agent filter to `agent_items`.
- `Layout ▾` also holds Timeline, Unstack, and Separate tags.
- Rows offer `✕` on hover; clicking a value edits it. People rows have no
  `✎`: changing a person is remove and add.
- Links cannot be removed: the ledger has no remove-citation call. Adding one
  is an API change for a later slice.
- The footer also carries the computed weight, and history loads only when its
  section is opened.

Verified: `./dev check`, and unit tests for the chips, the band colors, and the
date and duration labels. Not verified: anything in a running window, because
this session could not capture the screen. `docs/gui-graph-pickup.md` lists
what to try first.

Recorded by agent-assistant.

# Theme examples

Copy one of these to `theme.toml` in the configuration directory, next to
`config.toml`; the editor picks it up within a second, or use Settings (`,`) →
Reload from disk. A missing file means defaults; an unknown
key is reported rather than ignored, so a typo is visible.

Every file here sets only the keys it cares about. Anything left out keeps its
default.

## Visual channels

A node has three channels, and each is told what to stand for rather than being
wired to one meaning:

- `bar_encodes` — the accent bar on the leading edge
- `body_encodes` — the card fill, tinted down so text stays readable
- `outline_encodes` — the border

Each takes `status`, `tag`, `priority`, `weight`, or `none`. The default puts
status on the bar and the tag family on the body and border: where the work
stands, and what it belongs to, in one glance.

- `semantic-body.toml` — the default arrangement, stated explicitly
- `status-first.toml` — status everywhere, tags only in their regions
- `developer-2026-10.toml` — the developer's full tuned appearance, recorded
  as of October 2026 rather than promoted to the code defaults

## Window colors

```toml
[colors]
mode = "dark"                                   # dark (default), light, or system
palette_file = "~/.config/ghostty/themes/green" # optional Ghostty-format theme
```

`palette_file` takes any Ghostty theme file. Its `background`, `foreground`,
`selection-background`, and `cursor-color` restyle the panels, canvas, text,
selection, and links; card colors stay with the visual channels above. Because
the format is Ghostty's, one generated file can drive both the terminal and the
editor: point a matugen Ghostty template's output path here.

`mode = "system"` follows the operating system for the whole window. A light
palette colors the light appearance, and a dark palette colors the dark
appearance; the other appearance uses egui's built-in colors. Choose `dark` or
`light` in Settings → Colors to hold one appearance manually. A palette whose
background conflicts with that choice is not applied to the visible appearance.

The editor watches `theme.toml` and the palette file and reloads within about a
second of either changing, including a re-pointed home-manager symlink. If the
Settings window holds unsaved edits, it says the file changed instead of
discarding them; Reload from disk then applies the file, and Revert drops the
edits.

The family `+`/`-` and `-all` controls have three size modes under Settings →
Families. `as_is` keeps a fixed screen size, `with_card` follows the rendered
card width, and `clamped` keeps the fixed size within a configurable fraction
of card width. The default clamp is from `1/12` to `2/3` of the card width:

```toml
[parent]
control_scale = "clamped"
control_min_width_ratio = 0.08333333
control_max_width_ratio = 0.6666667
```

## Inspector band colors

```toml
[colors.inspector]
people = "#b48ce6"     # violet
time = "#e6b450"       # amber
structure = "#93aeda"  # the family connector blue
context = "#7fc28e"    # green
notes = "#8c8c8c"      # grey
```

Each colors only a thin stripe and the band label, never a fill behind text,
so any value keeps body text readable. A value that does not parse draws grey.

# Theme examples

Copy one of these to `theme.toml` in the configuration directory, next to
`config.toml`, and press Reload theme. A missing file means defaults; an unknown
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

The editor watches `theme.toml` and the palette file and reloads within about a
second of either changing, including a re-pointed home-manager symlink. If the
settings panel holds unsaved edits, it says the file changed instead of
discarding them; Reload theme then applies the file.

# Todo Graph

A dependency-free, local SVG viewer for the current Todo Ledger JSON export.
It has no server, package manager, or persistence layer.

## Use

Create a complete ledger view:

```sh
todo export graph --global --all --output /tmp/todo-graph.json
```

Open `index.html` in a browser and select that file. The browser's local file
picker keeps the ledger data on the machine.

## Current behavior

- Nodes are items, colored by state and ringed for high/critical priority.
- Dashed edges are parent-child relationships; arrowed solid edges are blockers.
- Project and tag controls filter the visible graph.
- Hover highlights direct parents, children, blockers, and dependents; click pins
  that selection.
- Drag to pan and use the toolbar or mouse wheel to zoom.

The viewer accepts only the versioned `todo-graph-v1` projection. It contains
the current `StoredItem` array and the project-ID-to-path map required for
accurate project layers.

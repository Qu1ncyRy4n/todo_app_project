# Graph Design Language

This is the shared vocabulary and visual contract for the todo graph. It is
intended to make design reviews and renderer changes refer to stable parts of
the picture instead of overloaded words such as “box,” “outline,” or “line.”

It records both current behavior and settled target behavior. Requirements
called out as **planned** are not claims about the current renderer.

## Reading order

The graph must communicate in this order:

1. task cards and parent/child ownership;
2. family structure and continuation;
3. dependency relationships;
4. tag context.

Tag clouds are ambient background context. They must never read as a family
frame or obscure the parent/child relationship.

## Terms

| Term | Meaning |
| --- | --- |
| **Task card** | The rendered todo itself: title, state/meta line, status, and tag accent. |
| **Tag cloud member** | The circle or rounded card-shaped background around one tagged task card. |
| **Tag rail** | The background connection joining nearby members with the same tag. |
| **Tag label** | The tag name attached to its cloud, not a label for a parent family frame. |
| **Parent outline** | The thick structural border that encloses a parent’s child space. It surrounds children, not the parent card. |
| **Parent tab** | A parent task card when it naturally touches the parent outline. |
| **Outline anchor** | A thick, direct bridge from a parent card to its parent outline when the card is surrounded by, or otherwise detached from, its child frame. It is not the parent outline itself. |
| **Child well** | The inner compartment bounded by the parent outline. |
| **Child infill** | Optional dark fill within the child well. It never leaks into the parent-card area or beyond the well boundary. |
| **Family trunk** | The segment from a parent card or outline anchor to a branch junction. |
| **Family junction** | An open-circle branch point in a family tree. |
| **Family branch** | An angular parent/child connector from a junction to a child. |
| **Family terminal** | The open circle at the child-side end of a family branch, placed at the top of a vertically arranged child card. |
| **Dependency edge** | A direct arrow for `depends_on`/blocking semantics. It remains distinct from the family tree. |
| **Hidden-child marker** | A small `…` at a parent card’s child-side edge. It appears only when children are hidden and reveals them on click. |

## Target family geometry contract

```text
             [ parent task card ]
                    │
                    │  outline anchor, only when detached
                    │
  ┏━━━━━━━━━━━━━━━━━┷━━━━━━━━━━━━━━━━━━┓  parent outline
  ┃                ○                   ┃  family junction
  ┃               ╱ ╲                  ┃
  ┃              ○   ○                 ┃  child terminals
  ┃            [A]   [B]               ┃
  ┃          child well / infill       ┃
  ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
```

The parent outline is much thicker than ordinary card and well strokes. Its
thickness and dark infill are bounded by the left and right ends of the parent
task card at the attachment point: a detached parent does **not** pull a thick
horizontal band across the whole child frame. The outline anchor carries that
connection instead.

When a parent card naturally attaches to the frame, it acts as the parent tab
and no separate outline anchor is drawn. The child infill begins below the tab;
it does not paint behind or past the card.

**Implementation status:** implemented. A detached parent uses a visibly thicker
Outline Anchor; a nearby parent uses a separate card-width attachment strip.
The broad child frame and dark infill remain bounded around the children.

## Family tree contract

Draw exactly one family path for each structural parent/child relationship.
Remove a duplicate diagonal when an angular family trunk/branch already carries
the same ownership relationship. Preserve a true dependency arrow between the
same tasks when one exists: it has different meaning and remains a separate,
direct arrow.

A multi-child family shares one trunk and junction, then fans out through
angular branches. Branches meet the top edge of their child card at an open
circle. They must not be replaced by several independent diagonal arrows.

The family-colored core may continue through the child well. The shared blue
casing/outline is visible only outside the child infill, where it separates the
family tree from the canvas. This is the **dynamic family core** rule.

**Implementation status:** implemented. The family-colored core remains
continuous; the shared blue casing is omitted for path segments whose midpoint
is inside a child well.

## Family presentation policy

The eventual policy has one inherited graph default and an optional override
per parent family:

| Policy | Meaning |
| --- | --- |
| **Dynamic** | Choose the compact bounded parent outline when containment helps; use an outline anchor for a detached parent; cap a remote continuation rather than drawing a giant slab. |
| **Box** | Force the bounded parent-outline reading for that family. |
| **Tree** | Suppress the parent outline and show only the family trunk, junctions, branches, and terminals. |
| **Inherit** | Use the graph default. This is the default for each family. |

The current renderer has a graph-wide theme default plus a right-click
per-parent override. `Inherit` removes the local entry; `Dynamic`, `Box`, and
`Tree` persist in `gui-layout.json`, are presentation-only, and affect only
that family's frame/capping behavior. Box frames remain bounded rather than
allowing a remote child to create an unlimited slab.

## Tag cloud contract

Each task contributes one cloud member from its deepest effective tag. A cloud
uses fully pre-blended color so overlapping members and rails of the same tag
remain one continuous color rather than darkening. Different tag clouds stay
visibly separate.

Cloud members may be circular or card-shaped. Card-shaped members follow the
measured card rectangle, including wrapped-title height, with configurable
padding. They annotate cards; they do not form a child well or parent outline.

## Appearance Lab

The proposed **Appearance Lab** is an optional renderer page using a fixed,
synthetic exemplar graph and the production theme/renderer. It should expose
the same live appearance state as the real graph, without writing ledger data.
Its exemplar scenes should include:

1. an isolated tagged task and a same-tag pair;
2. overlapping different-tag clouds;
3. a parent with multiple children and a shared junction;
4. a nested family with a detached parent and outline anchor;
5. a remote child/continuation board;
6. a family relationship plus a real dependency edge between the same cards;
7. wrapped titles, selected cards, focus, and hidden descendants.

It is available from the toolbar as **Appearance Lab**. It is deliberately
ledger-free and uses the live theme values, so presentation controls can be
tuned without changing real task data.

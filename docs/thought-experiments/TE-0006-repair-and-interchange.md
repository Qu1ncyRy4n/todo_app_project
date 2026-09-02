# TE-0006: Safe Repair and Plain-Text Interchange

Date: 2026-08-28
Status: selected

## Pressure

Dogfooding revealed that capture without correction makes ordinary mistakes
expensive, while one-process-per-item imports are slow and can stop halfway.
The ledger must remain append-only without making the current view immutable.

## Repair routes

### A: Rewrite original events

Simple views, but destroys provenance and invalidates content IDs. Rejected.

### B: Append correction events and update the materialized view

Title, project, parent, and namespace corrections are events. The current item
row changes in the same transaction. Selected.

Project moves reject items with structural or blocking relationships crossing
the move boundary. Users must repair those explicit relationships first.

## Interchange routes

### A: One universal lossless text format

Attractive in theory, but makes ordinary Markdown difficult to edit and asks
one representation to be both human shorthand and durable protocol.

### B: Formats with explicit fidelity

Selected:

- Markdown is a readable checkbox-tree view and import format.
- JSON is a rich current-view export for tools.
- CBOR is an array of the exact immutable event-envelope byte strings.

Markdown import recognizes `[ ]`, `[>]`, `[x]`, and `[-]`, derives parents from
indentation, validates the complete document, then inserts the batch in one
transaction. `--dry-run` performs parsing and rendering without writes.

## Decision

Select repair route B and interchange route B. Lossiness is documented rather
than hidden. A later sync/import protocol may consume exact CBOR archives.

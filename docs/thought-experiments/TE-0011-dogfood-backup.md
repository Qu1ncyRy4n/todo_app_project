# TE-0011: Dogfood Backup Before Complete Replay

Date: 2026-09-04
Status: selected

## Pressure

Fast dogfooding is worth some compatibility breakage, but captured item titles,
completion state, and hierarchy must remain recoverable through ordinary CLI
commands before exact event replay exists.

## Routes

### A: Copy only the live database

Highest local fidelity, but unsafe if copied during writes and coupled to the
current Turso schema. It does not provide an inspectable CLI recovery route.

### B: Export only exact CBOR events

Preserves durable bytes, but the current CLI cannot import or project them.

### C: Export only global Markdown

Immediately importable, but loses project scopes and all richer facts.

### D: Create a tiered recovery directory

Write one importable Markdown tree per scope, a rich global JSON view, and the
exact CBOR event archive into a new directory. Include restore instructions and
state the fidelity limits. Selected.

## Decision

Select route D. Refuse to reuse an existing destination directory so a backup
cannot silently mix with or overwrite another generation. Markdown recovery
preserves item titles, states, and parent hierarchy. JSON and CBOR retain richer
diagnostic evidence until their import/replay paths exist.


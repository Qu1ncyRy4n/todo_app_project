# DI-0009: Tiered Dogfood Backup

Date: 2026-09-04
State: accepted
Source: TE-0011

`todo backup DIR` creates a new recovery directory containing importable
Markdown trees separated by project scope, a rich global JSON current view, an
exact CBOR event archive, and generated restore instructions.

The command never writes into an existing destination directory. Markdown is
the minimum immediately recoverable tier and preserves title, lifecycle state,
and parent hierarchy. It does not preserve IDs, namespaces, focus metadata,
notes, citations, dependencies, or event history. JSON and CBOR preserve more
evidence but remain export-only until their accepted import and replay paths are
implemented.

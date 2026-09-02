# DI-0006: Append-Only Repair and Tiered Export Fidelity

Date: 2026-08-28
State: accepted
Source: TE-0006

v0.2.1 adds title edits, parent clearing, namespace removal, guarded project
moves, transactional Markdown checkbox-tree import, and three export views.

All repairs append v3 events and update views atomically. Markdown import is
all-or-nothing after full validation. Markdown is intentionally human and
lossy, JSON represents the current view, and CBOR preserves exact event bytes.

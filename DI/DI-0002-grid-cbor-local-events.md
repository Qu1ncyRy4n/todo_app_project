# DI-0002: Project-Local Grid-CBOR Events

Date: 2026-08-27
Status: active
Decision: Store immutable project-local Grid-CBOR envelopes with pCID, parents,
payload, and a null v0.1 proof slot. Store extracted fields as rebuildable views.
Intent: Preserve exact history and PG migration space without claiming current
PG compatibility.
Source: TE-0002.


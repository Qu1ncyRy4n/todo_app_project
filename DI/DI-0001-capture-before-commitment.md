# DI-0001: Capture Before Commitment

Date: 2026-08-27
Status: active
Decision: Captured items begin in `inbox`. Explicit activation moves them to
`active`; `ongoing` is a CLI alias. Capture is not a formal promise.
Intent: Keep capture cheap while preserving a future explicit commitment layer.
Source: TE-0001.


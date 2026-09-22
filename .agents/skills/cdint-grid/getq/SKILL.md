---
name: getq
description: Receive and handle at most one request from the current cdint-grid worker's local worker inbox. Use whenever the user says exactly `getq` or asks this worker to process one queued peer request.
---
<!--
TBD: Imported verbatim from captured cdint-grid skill getq for QMR dogfooding.
This procedure may require cdint-grid executables, worker branches, identity rules,
coordination files, or organization decision records that are not configured in a
QMR consumer. Do not treat this import as approval or portable default policy.
-->

# Get One Worker Request

The worker inbox carries voluntary requests. Delivery, storage, or selection
does not compel acceptance and does not expand the worker's current authority.
Source: DI-jirob; DI-nunan; DI-josov; DI-fufav; DI-ruhog; DI-sulad;
DI-bidoz

## Receive Once

1. From the current worker's worktree, run
   `/home/stevegt/bin/cdint-grid-worker-inbox getq` exactly once. Do not loop,
   poll, sleep, or process a second request in the same invocation.
   Ordinary retrieval processes unread supported carriers by recipient-side
   carrier modification time, oldest first, before eligible local-ready
   requests. Missing parents are reported but do not make a request ineligible.
   Treat the carrier timestamp only as disposable local scheduling input, never
   as event order or durable evidence. Use `--lifo` only when explicitly
   requested.
2. Preserve the returned CID and complete JSON result. If the command returns
   JSON `null`, report that no request is ready and stop.
3. Ordinary `getq` selects a supported ready request itself. Use `pick CID`
   only to choose a different exact ready request or to store an unknown-pCID
   message explicitly; do not execute unknown protocol meaning.
4. Treat the selected body as a deferred user request under the current
   `AGENTS.md`, governing TODO or DR, locked decisions, path approvals,
   barriers, side-effect authority, and stop conditions.
5. Use the separate `drain-inbox` skill when the user requests one finite
   whole-inbox handling round. Do not turn this one-message procedure into a
   loop.

## Record The Outcome

- Use `complete CID` only after the request has a final locally promised
  outcome. Completion means the worker handled the request, not that it accepted
  every proposal in the body.
- Use `block CID --reason TEXT` only when a named dependency prevents a final
  outcome. Use `retry CID` when that dependency is resolved.
- On an ambiguous failure, leave the request selected and report the ambiguity.
- Never infer work acceptance from a receipt, wake or control another worker
  through inbox traffic, or delete inbox CAS evidence automatically.

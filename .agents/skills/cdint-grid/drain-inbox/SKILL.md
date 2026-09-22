---
name: drain-inbox
description: Turn one stable cdint-grid worker-inbox snapshot into durable TODO work. Use when the user says `drain-inbox`, asks to process the current inbox backlog, or asks to map all currently actionable peer requests to governing TODOs without chasing later arrivals.
---
<!--
TBD: Imported verbatim from captured cdint-grid skill drain-inbox for QMR dogfooding.
This procedure may require cdint-grid executables, worker branches, identity rules,
coordination files, or organization decision records that are not configured in a
QMR consumer. Do not treat this import as approval or portable default policy.
-->

# Drain One Inbox Snapshot

Process one finite view through existing single-message operations. The inbox
remains durable message evidence; TODO files remain the work authority.
Source: DI-bidoz; DI-sulad; DI-vilih; DI-rulak; DI-pidoz; DI-vunuj

## Freeze The Round

1. From the current worker's worktree, run
   `/home/stevegt/bin/cdint-grid-worker-inbox list` exactly once.
2. Freeze the `pending` and `ready` CIDs from that output only when the displayed
   recipient is the current worker. The local list also shows requests this
   worker sent to peers; do not pick those outbound request states. Deduplicate
   repeated CIDs while preserving first appearance. Ignore `selected`,
   `blocked`, and `completed` entries for this round.
3. Process only that frozen set. Do not list again, poll, sleep, or include a
   message that arrives later.

## Read Both Coordination Protocols

1. Treat the installed executable as the active wire behavior. Tracked
   successor guidance does not by itself prove that main qualified, installed,
   or cut over the successor binary. Source: DI-ladag
2. After the qualified cutover, handle supported original and successor
   coordination messages by their request, receipt, or work-state meaning.
   Newly authored receipts and work states use the successor pCID; never rewrite
   an older message merely because it uses the original pCID.
3. Treat parent order as deterministic representation, not trust, acceptance,
   semantic precedence, or winner selection. Continue reporting missing parents
   as incomplete local closure. Do not add stronger cross-protocol, sender,
   recipient, direction, or timeline checks here; `TODO-humop` owns that later
   validation. Source: DI-ladag

## Process Each Message

For each frozen CID in order:

1. Run `/home/stevegt/bin/cdint-grid-worker-inbox pick CID` exactly once and
   preserve the complete JSON result.
2. Treat `missing_parents` as an honest incomplete local view, not a selection
   failure. Do not fetch, interpret, or invent the missing blocks.
3. A supported actionable request maps to exactly one governing TODO. Search
   current TODOs first. If one exactly owns the work, add the request CID there.
   Otherwise follow decision-first naming and path approval, mint a handle,
   create one TODO, and index it in `TODO/TODO.md`.
4. An unknown-pCID message is stored only as opaque evidence. Create or update
   one TODO to obtain and evaluate its handler; never guess or execute its body.
5. A receipt is storage evidence only. Report it without creating a TODO merely
   for its existence.
6. A `selected` or `ready` work-state notice is status evidence unless its body,
   age, or governing barrier exposes a separate action. Record it against the
   original request and governing TODO when known.
7. A `completed` work-state notice requires follow-up. Locate the worker's exact
   result and governing TODO, and record the required local evaluation or
   integration work there. If the result cannot yet be found, record that exact
   missing evidence. Do not integrate the result during the intake pass; the
   worker's completion does not imply local acceptance.
8. A `blocked` work-state notice also requires follow-up. Read its complete
   reason and map the blocker-removal or decision work to its governing TODO or
   DR. Do not resolve the blocker during the intake pass, and do not leave the
   notice as passive evidence.
9. Do not create a duplicate TODO solely because a completed or blocked notice
   exists. Update the TODO that owns the original request, or create one only
   when no current TODO owns the required integration or blocker resolution.
10. A message requiring a material decision maps to its governing DR or TODO;
   continue the frozen snapshot without making that decision. Stop only when an
   operational error or ambiguity prevents safe classification. Keep all prior
   picks and TODO mappings and report the stopping CID and unprocessed remainder.

## Finish The Round

- Do not run `complete CID` merely because a TODO was created or updated. The
  request remains selected until its requested work has a final outcome under
  the governing TODO.
- Report every picked CID, its classification, its governing TODO when
  actionable, every completed-result follow-up, every blocked condition and
  required response, missing parents, and any frozen CIDs left unprocessed.
- Produce one list item for every frozen inbound message. Each item includes
  its CID, sender, message classification or work state, and governing TODO or
  DR. Use `none` plus a short reason when no assignment is required. Do not
  omit receipts, duplicates, legacy translations, or status-only notices.
- After the list, report counts for the complete frozen inbound set by message
  kind and work state, TODO or DR assignment, completed-result follow-up,
  blocked follow-up, unresolved mapping, processed messages, and any unprocessed
  remainder. For a large round, retain the complete list in a mode-0600 private
  report and give its path plus the statistics in chat.
- After mapping the complete frozen snapshot, prioritize all resulting TODO and
  DR work using current critical-path, safety, dependency-unblocking, and
  integration evidence. Do not begin that work during the drain.
- Exact `getq` remains the procedure for handling at most one request. This
  skill does not add or emulate a batch mutation command.

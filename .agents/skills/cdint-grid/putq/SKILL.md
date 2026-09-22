---
name: putq
description: Send one bounded voluntary request to a named cdint-grid worker through the worker inbox. Use whenever a direct user message begins with `putq ` or a worker needs to offer work or information to a peer.
---
<!--
TBD: Imported verbatim from captured cdint-grid skill putq for QMR dogfooding.
This procedure may require cdint-grid executables, worker branches, identity rules,
coordination files, or organization decision records that are not configured in a
QMR consumer. Do not treat this import as approval or portable default policy.
-->

# Put One Worker Request

Queueing a request makes it available to another worker. It does not grant
authority, reserve a task, force acceptance, or make the sender's assessment
globally true. Source: DI-jirob; DI-nunan; DI-josov

## Choose Recipient And Body

1. For a direct user message beginning with the five characters `putq `, use
   `main` as the recipient and use the exact nonblank remainder as the body.
   Do not process that deferred body in the same turn.
2. For an explicit peer request, require one stable worker branch name as the
   recipient and one bounded nonblank body. Prefer exact Git commits, TODOs,
   DRs, CIDs, and stop conditions over copied context.
3. Never put credentials, secrets, or unnecessarily large content in the body.
   Name a protected file or durable Git object when appropriate.

## Preserve Outbound Histories

1. Treat the installed executable as the active wire behavior. The tracked
   successor decision does not by itself prove that main qualified, installed,
   or cut over the successor binary. Do not announce live successor behavior
   without that evidence. Source: DI-ladag
2. After the qualified cutover, ordinary authorship uses the successor pCID and
   joins every locally known outbound head for the sender and recipient across
   the old and successor coordination protocols. Never choose one competing
   head, delete the others, or rewrite historical messages merely to make a send
   succeed.
3. Treat parent ordering as deterministic representation only. It does not
   identify a trusted parent, accepted history, semantic winner, or global
   timeline. Missing parents remain reportable, and `TODO-humop` owns stronger
   semantic parent validation. Source: DI-ladag

## Report A Main-Owned Stop

1. When the current worker must stop because `main` must make a decision,
   provide evidence, integrate work, or perform another named action, send one
   request to recipient `main` before waiting. Source: DI-fobit
2. State the stable worker name, governing TODO or DR, exact action requested
   from `main`, the point the worker cannot cross, and the exact evidence needed
   to resume. Keep the body bounded and use Git commits, CIDs, and tracked paths
   instead of copying large evidence.
3. Record the returned request CID and ask once in the worker's local session.
   Do not poll `main` or resend an unchanged request. A materially changed
   blocker may use a new request that identifies what changed.
4. If sending fails, record the exact failure and remain stopped. After an
   uncertain send, inspect retained CAS and transport evidence before deciding
   whether any retry is safe.

## Send Once

1. From the sender's worktree, pass the exact body on standard input to
   `/home/stevegt/bin/cdint-grid-worker-inbox putq RECIPIENT` exactly once.
2. Report the returned message CID and recipient. Do not retry blindly after an
   uncertain result; inspect the sender's local CAS and request status first.
3. Do not poll the recipient, inject the body into its TUI, or infer acceptance
   from delivery or receipt evidence.

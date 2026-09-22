---
name: next10
description: Analyze cdint-grid and list its next ten priorities. Use this skill whenever the user says `next10` or asks for the next ten project tasks, including critical-path, queue, and current-discussion reconciliation.
---
<!--
TBD: Imported verbatim from captured cdint-grid skill next10 for QMR dogfooding.
This procedure may require cdint-grid executables, worker branches, identity rules,
coordination files, or organization decision records that are not configured in a
QMR consumer. Do not treat this import as approval or portable default policy.
-->

# Next 10

Produce one current, evidence-based priority list without changing the project
plan or queue while ranking it.

## Workflow

1. Re-read the current repository `AGENTS.md` and `GLOSSARY.md`.
2. Inspect the active dobab deadline record. Read its component milestones,
   dependency matrix, Daily Calendar, checklist, and linked TODO or DR evidence
   needed to understand the critical path and prerequisites.
3. Run `/home/stevegt/bin/cdint-grid-worker-inbox list -v` exactly once from
   main's worktree. Use its derived states, CIDs, and complete bodies.
4. Review the current chat context for decisions, plans, or work that have not
   yet been persisted. Do not claim access to unavailable or compressed history.
5. Deduplicate overlapping plan, queue, and chat items. Prioritize real launch
   safety failures and missed justified operational requirements, then their
   diagnostics and critical-path implementation, critical-path integration,
   already-claimed work, and work that unlocks several dependents. A missed
   arbitrary experiment cutoff must be reported literally but is not a launch
   blocker or optimization target.
6. Assess current housekeeping needs, including disk-space reduction,
   productivity improvements to skills, tools, or processes, cleanup of
   unneeded workers, and token-use reductions to skills, tools, or processes.
   Include one or more evidence-based housekeeping priorities in every result,
   and include each category whose urgency places it among the actual top ten.
   For every included housekeeping priority, state the specific recommendation
   and concrete, actionable implementation steps. Do not emit a generic
   housekeeping label or invent work only to satisfy this requirement.
7. Output exactly ten human-readable priorities as one Markdown numbered list.
   Identify the relevant queue item ID and state when a priority came from the
   queue. Put no blank source lines between adjacent list items. Keep each item
   compact while preserving every required recommendation and actionable step.

Do not claim or mutate any queue item merely because it appears in the ranking.

## Starting Inbox Work

Before beginning work on a listed inbox request, run:

```text
/home/stevegt/bin/cdint-grid-worker-inbox pick CID
```

Process only the exact body returned by `pick`, under the current instructions
and authority. Do not substitute `getq` when the item was selected by CID.

## Selected Request Cleanup

Before ranking the ten priorities, inspect selected requests for evidence that
work is genuinely complete. Represent each selected request that appears ready for
cleanup as one of the ten numbered priorities, naming its CID, evidence, and the
need for the user's explicit confirmation before running `complete CID`.
These cleanup priorities consume ordinary list positions; do not append a
separate post-list confirmation question. If the evidence is uncertain, rank
the remaining work rather than claiming that the request is complete. Never run
`complete` without the user's explicit confirmation.

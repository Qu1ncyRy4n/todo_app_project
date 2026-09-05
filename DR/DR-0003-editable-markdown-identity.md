# DR-0003: Editable Markdown Identity and Authority

Date: 2026-09-04
State: deferred
Question: Should an editable ledger projection use readable stable slugs,
Obsidian block IDs, Markdown links, HTML comments, a sidecar mapping, or a
`TODO/TODO.md` index with detail files to preserve item identity?
Why this does not block current dogfooding: the event ledger and CLI remain the
single authority, and existing Markdown import/export can be used as explicitly
lossy interchange. Real project use should reveal the most valuable edit and
layout operations before the durable representation is selected.
Safety constraint: a future editable projection must preview and approve a
semantic change plan, reject or merge stale edits, and never interpret a
missing Markdown line as automatic deletion.
Source: TE-0009.


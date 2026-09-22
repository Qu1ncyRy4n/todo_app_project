---
name: run-thought-experiment
description: Load exact governing evidence and run a scenario-based thought experiment before decision framing. Use when AGENTS.md or a task requires a TE, alternatives need failure analysis, or a proposed architecture must be narrowed before user decisions.
---
<!--
TBD: Imported verbatim from captured cdint-grid skill run-thought-experiment for QMR dogfooding.
This procedure may require cdint-grid executables, worker branches, identity rules,
coordination files, or organization decision records that are not configured in a
QMR consumer. Do not treat this import as approval or portable default policy.
-->

# Run Thought Experiment

The deterministic tools bound source loading; the thought experiment still
performs the required scenario analysis and records unresolved conclusions.
Source: DI-sipog; DI-pugid; DI-hasod

## Workflow

1. Identify the exact TODO task, relevant DI IDs, glossary terms, and named
   AGENTS sections.
2. Load them with `cdint-grid-token-efficiency todo`, `glossary`, and
   `agents section`.
   Do not dump whole coordination documents when an exact selector works.
3. Verify premises against source or code and label each as locked, proposed,
   experimental, or unresolved.
4. Compare the real alternatives through normal, failure, recovery, privacy,
   compatibility, and rollback scenarios required by the task.
5. Write the TE at its approved path, record conclusions without pretending
   they are decisions, then ask the required decision question and append the
   resulting DI before implementation.

Expand a named source section when evidence is missing. Never fill a gap with a
model-generated summary presented as authority.

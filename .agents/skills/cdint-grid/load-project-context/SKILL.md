---
name: load-project-context
description: Load exact, bounded repository context through deterministic local tools. Use for GLOSSARY terms, TODO task and DI context, AGENTS guidance fingerprints or sections, and calendar image-view decisions instead of broad whole-file reads.
---
<!--
TBD: Imported verbatim from captured cdint-grid skill load-project-context for QMR dogfooding.
This procedure may require cdint-grid executables, worker branches, identity rules,
coordination files, or organization decision records that are not configured in a
QMR consumer. Do not treat this import as approval or portable default policy.
-->

# Load Project Context

Use the smallest command that answers the named question. The tools validate
their complete source locally and emit exact Markdown with line provenance;
their output is not an LLM summary. Source: DI-sipog; DI-pugid; DI-hasod

## Workflow

1. Use the main-installed `cdint-grid-token-efficiency` executable. Main
   rebuilds and atomically installs it when its source changes; workers do not
   compile private copies. Source: DI-kolat
2. Select the narrow operation:
   - vocabulary: `cdint-grid-token-efficiency glossary --term TERM`;
   - task and active DI closure: `cdint-grid-token-efficiency todo --source PATH --task TASK`;
   - one DI: `cdint-grid-token-efficiency todo --source PATH --decision DI-ID`;
   - AGENTS fingerprint: `cdint-grid-token-efficiency agents manifest`;
   - changed guidance: `cdint-grid-token-efficiency agents verify --expected-commit COMMIT --expected-blob BLOB`;
   - one guidance section: `cdint-grid-token-efficiency agents section --name HEADING`;
   - PNG policy: `cdint-grid-token-efficiency calendar-view --base-commit COMMIT`.
3. Read the emitted exact view. Expand only by another named term, task,
   decision, section, or explicit defect. If validation fails, inspect the
   named error; do not fall back automatically to dumping the whole source.

Never use Ollama or another model as validation authority.

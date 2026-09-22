---
name: update-dobab
description: Load and validate bounded operational context for a complete TODO-dobab reconciliation. Use whenever the user says update dobab, during date rollover, or when current deadline state must be reviewed without sending all closed history to the model.
---
<!--
TBD: Imported verbatim from captured cdint-grid skill update-dobab for QMR dogfooding.
This procedure may require cdint-grid executables, worker branches, identity rules,
coordination files, or organization decision records that are not configured in a
QMR consumer. Do not treat this import as approval or portable default policy.
-->

# Update Dobab

This skill reduces context loading; it does not reduce any requirement of the
canonical TODO-dobab update protocol. Source: DI-hasod; DI-kamam

## Workflow

1. Use the main-installed `cdint-grid-dobab-calendar` command. Main rebuilds and
   atomically installs it when source changes; workers do not compile private
   copies. Source: DI-kolat
2. Run `cdint-grid-dobab-calendar context`. If the only failure is the expected
   current-date/status mismatch during an actual rollover, rerun with
   `cdint-grid-dobab-calendar context --rollover`.
3. Use the exact operational view for the deadline, milestones, dependency
   coverage, failure rules, open checklist branches, newest closed day, and
   active/future days. The command validates every supported top-level section
   against one exact ordered classification but intentionally omits the Table Of
   Contents and Decision Intent Log from routine output.
4. Fetch a governing decision only when needed:
   `cdint-grid-token-efficiency todo --source TODO/TODO-dobab-august-osc-qb-production.md --decision DI-ID`.
5. Fetch older closed history only when rollover evidence or a named fact needs
   it: `--closed-date YYYY-MM-DD` or `--closed-match 'literal source text'`.
6. Gather the remaining Git, test, target, and current-time evidence required by
   AGENTS.md, edit the authoritative source, regenerate the calendar, and run
   all required audits.

Do not accept partial stdout after any validation error, including an unknown,
missing, duplicate, or misplaced top-level section. Do not show the PNG to the
model for routine source or annotation updates; exact-byte and Graphviz checks
still run. View it only for generator/render behavior changes or an explicitly
suspected visual defect.

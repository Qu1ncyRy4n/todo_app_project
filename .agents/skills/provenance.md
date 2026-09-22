# Skill Provenance

This file records the source history and review boundaries for QMR skills. It
is not a skill and should not be loaded as a procedure.

## Rust Development

- Skill: `rust-development/SKILL.md`
- Source: `archive/agents-libraries-v1/personal/lang/rust.md`
- Review boundary: repository wrappers and project-specific checks take
  precedence. Dependency, runtime, and compatibility decisions remain explicit
  target decisions.

## Nix Development

- Skill: `nix-development/SKILL.md`
- Source: `archive/agents-libraries-v1/personal/lang/nix.md`
- Review boundary: development-shell changes and system-activation work are
  distinct. Host commands and secret mechanisms are target-specific.

## Shell Command Failures

- Skill: `shell-command-failures/SKILL.md`
- Source: `archive/agents-libraries-v1/cdint/engineering/error-handling.md`
- Review boundary: Go-specific error-handling guidance was not imported; use a
  language-specific skill or target check where applicable.

## Git Change Finalization

- Skill: `git-change-finalization/SKILL.md`
- Sources: `archive/agents-libraries-v1/cdint/shared-baseline/instructions/git.md`,
  `agents/constraints/focused-change-loop.md`, and
  `agents/constraints/unknown-work-caution.md`
- Review boundary: commit authority follows the selected Git commit mode. The
  skill does not authorize external Git actions or unrelated-work staging.

## Maintain Documentation

- Skill: `maintain-documentation/SKILL.md`
- Sources: `archive/agents-libraries-v1/cdint/shared-baseline/instructions/documentation.md`,
  and `archive/agents-libraries-v1/cdint/docs/public-prose.md`
- Review boundary: record identifiers and public provenance are target
  conventions. Do not expose internal records in public material unless the
  target requests it.

## Preserve Comment Intent

- Skill: `preserve-comment-intent/SKILL.md`
- Source: `archive/agents-libraries-v1/cdint/engineering/comment-intent.md`
- Review boundary: decision-record references are target conventions. Add them
  only when they preserve otherwise-lost behavior rationale.

## Edit Thought Experiment

- Skill: `edit-thought-experiment/SKILL.md`
- Source: `archive/pre-qmr-structure/skills/edit-thought-experiment/SKILL.md`
- Review boundary: the target repository supplies TE statuses, record layout,
  and ID convention. The archived Cat-1a through Cat-7 vocabulary is evidence,
  not required QMR policy.

## Captured CDINT Grid Skills

- Source root: `reference/source-guides/cdint-grid_skills/`
- Destination: `skills/cdint-grid/`
- Import: all 18 procedures are verbatim imports plus HTML TBD notes.
- Review boundary: CDINT-specific executables, worker coordination, identities,
  and decision records are not QMR authority or portable policy; inspect and
  dogfood only until dependencies are explicitly configured.

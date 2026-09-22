---
name: preserve-comment-intent
description: Preserve explanatory comments through code changes and add intent comments only for non-obvious behavior. Use when refactoring or changing parser, encoding, rendering, concurrency, storage, protocol, or recovery logic.
---
# Preserve Comment Intent

## Preserve Existing Explanation

Do not remove an explanatory comment unless the same change replaces it with
equal or better explanation near the same logic. During a refactor, preserve the
old intent before improving its wording.

## Explain Intent, Not Syntax

For non-obvious parser, encoding, rendering, concurrency, storage, protocol, or
recovery logic, add a short plain-English comment explaining why the behavior
exists. Do not add comments that merely restate the code.

## Audit The Comment Diff

Review removed and added comments in the focused diff before finalizing. In a
repository that uses decision records, add a behavior-level record reference
only when it helps a future maintainer recover otherwise-lost intent.

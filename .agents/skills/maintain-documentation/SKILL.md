---
name: maintain-documentation
description: Update repository documentation, public prose, and design records so they remain accurate, scoped, and useful. Use when changing README files, guides, specifications, or other reader-facing material.
---
# Maintain Documentation

## Keep Related Material Consistent

Update design records, implementation, and public documentation together when a
behavior change requires it. Do not rewrite unrelated prose for style.

## Write For The Intended Reader

Use direct statements, concrete examples, and short paragraphs for public
technical prose. For durable specifications, state definitive requirements and
avoid filler. When uncertainty matters, state what is unknown, why it matters,
and where the open question is tracked.

Keep public slides and reader-facing documents free of internal DR, DI, TODO, or
TE references unless the target repository intentionally exposes that
provenance.

## Verify

Review documentation and comment diffs with the implementation diff. Where a
repository uses decision records, add a behavior-level reference only when it
helps a future maintainer recover non-obvious intent.

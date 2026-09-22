

# Atomic structure

Three categories of data:
- Nodes (things with content): Descriptions, todo items, roles, goals MOC. (dates?)
- Tags: Roles, categories, projects etc. Certain cannonical things are aliased as tags (appear as just themselves: roles, goals, projects) in ui but are stored as tags.
- Links: links to items

This Nodes / Tags / Links model is the shared primitive across the exocortex
tools: nodes are **atoms**, tags and links form emergent clusters (**clouds**).
Sources, notes, tasks, roles, goals, and projects are all nodes.

---

# Shared substrate (with teacher-bot)

See `teacher-bot_project/spec.md` for the full picture. Points that belong here
because they are todo-side:

## Roles / goals / values / projects as referenceable nodes

- These are declared, hand-edited nodes — the user's own structure, not inferred.
- They are authored here and **referenced** by the teacher bot (and any other
  exocortex tool) by link, so a tool can ground its behavior in the user's real
  roles, goals, and values without re-asking or guessing.
- This is the primary near-term integration between the two apps.

## Content-addressed store + version DAG

- Node content is content-addressed by hash (PromiseGrid CID idea).
- Mutation: immutable content blobs, a mutable human-readable pointer per node
  (name → current hash, moved on edit), and a parent-linked version DAG.
- Same scheme the teacher bot uses for sources and notes; shared, not duplicated.

## Source / document linkage

- A task or node can link to source material or docs via Obsidian-style
  links. In-vault attachments use wiki-links (`[[file.pdf]]`,
  `[[file.pdf#page=3]]`); files outside the vault use Markdown file links
  (`[label](file:///abs/path)`) until an out-of-vault linking story is settled.
- This lets tasks point at the reading/source that motivates them, and lets the
  teacher bot surface tasks tied to a source.

## Shared libraries, not a shared runtime

- Shared: the data model/format, the content-addressed store, and the
  TTS + Markdown-to-speech normalizer library (todo uses it for read-aloud).
- Not shared in v0: no common process or framework. Coupling is by format and by
  link. Revisit only if duplication actually hurts.

## Interface path

CLI now (current Rust binary); close/desktop GUI (Tauri) later — same path as the
teacher bot.




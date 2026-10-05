# Dogfooding Direction

## Primary question

Dogfood the ledger from high-level goals down to concrete next actions. The
product succeeds when a person can keep a meaningful goal connected to its
supporting work and still identify what to do next without losing context.

Start with general life, learning, or creative goals rather than implementation
work for this repository. Project-specific sample tasks hide whether the model
works outside software development.

## Current dogfooding shape

Create a small number of meaningful roots, then decompose each only as far as
needed for the next action:

```text
Improve household resilience
|- Understand current monthly costs
|- Choose one affordable improvement
`- Complete the chosen improvement

Build a sustainable learning practice
|- Choose one topic
|- Schedule the first session
`- Record what helped or blocked progress
```

Use parents for goal-to-action decomposition, notes for context discovered
while working, and blockers only for genuine prerequisites. Avoid creating a
large project plan before real use shows that its children are needed.

## Working-set Scenario To Validate

The next dogfood cycle should observe this common sequence:

1. Begin work on `x`.
2. Capture `y` as work to consider after `x`.
3. Discover that `z` blocks `x`, so bring `z` forward.
4. Complete `z`, then resume or finish `x`.
5. Get distracted by `a`, decide it is not urgent, and place it after the
   current interruption rather than letting it displace current work.

This is evidence for a future working-set and insertion model. The terms
"deque", "front", and "after" are intentionally not API commitments yet.
Dogfooding must establish whether people need an ordered queue, a smaller
currently-working set, a relationship between items, or some combination of
these before durable ordering state is introduced.

## Near-Term Product Questions

- Hierarchy: does parent/child decomposition make the connection from a goal to
  a next action visible enough?
- Interruptions: can a person record a blocker and a non-urgent distraction
  without losing the intended sequence of work?
- GUI: evaluate a GUI early enough to test whether a visual hierarchy and
  working set make these relationships easier to inspect and manipulate than
  CLI output. The first GUI should make its state changes observable and use
  the same ledger APIs; its interaction model remains open.

Record concrete friction from these scenarios before deciding a queue data
model, insertion commands, drag-and-drop behavior, or GUI architecture.

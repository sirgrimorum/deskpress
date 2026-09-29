# 0019: Group, the one component that holds others

Status: accepted, 2026-09-28.

## Context

The documents view of the design is one card per person with that person's documents inside. A
layout was flat: a component repeats with `each`, but none holds others, so a card around rows could
not be written. Headings between rows would have looked close without being it.

## Decision

**`Group` holds a `layout` of its own.** `Group: {each?, if?, title, layout: [components]}` draws a
titled box around what its layout draws. Any component can go inside, another `Group` too. A Group
with nothing drawn inside is left out, as a Card with nothing to say is.

**`group` names the item inside.** A repeating Group's item is `item` in its own props and `group`
inside, so a child that repeats with its own `each` still reaches it, as in
`Row: {each: documents, if: "item.for == group.id"}`. Both names are only known where they apply.

**The tree carries `children`.** A node gets `children`, empty for every kind but `Group`. That
changes the shape of the tree, so its version goes from 3 to 4, and a renderer on 3 refuses it
instead of drawing a Group as nothing.

## Consequences

- The set of components is 13, one more than the design's twelve. The Android renderer draws a Group as a card and its children as
  they would be outside; the design system screen shows one.
- Grouping by a key is done with `if` inside the Group, which reads the whole list once per group.
  For the lists a pack holds (a handful of people, a few dozen documents) that is nothing; a
  `group_by` would be the next step if a list ever grows past that.

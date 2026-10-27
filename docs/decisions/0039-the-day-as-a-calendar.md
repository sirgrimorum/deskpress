# 0039: the day as a calendar

Status: accepted, 2026-10-27. Amends 0025, 0032 and 0035.

## Context

On the trip, moving the day about did not work. Each row was as tall as it lasted within limits,
so the scale changed from row to row and a drag read differently on each. A block with no `until`
lasted to the next one, so free time never showed. Handles sat on blocks the engine refuses to
move. When the family split, both people's blocks were one line, and a move pushed the other
person's hours. And an edit was keyed by the block's place in its list, so a newer pack with one
block more moved the edit onto the wrong block.

## Decision

**The Day is a calendar.** One scale for every hour, a ruler of hours on the left and a now line.
A block is as tall as it lasts, at least fifteen minutes. What is over is not drawn; the ruler
starts at the hour of the first block still to come. Blocks that overlap share their column in
lanes, and a leg is drawn behind, muted.

**A column per person when the day splits.** The engine gives each timed entry with a `for` a
`track`, the place of its set of people in the day's `tracks`, when there are two sets or more.
`tracks` names each set. A block with no `for` crosses every column.

**A block's own track moves.** A move or resize reshapes only the blocks that share a person with
the one dragged, and the ones for everyone. A leg is worked out from the last block on the same
track, and a block lasts to the next one it shares a person with.

**`fixed` says it cannot move.** A `locked` block, or one that ends on another clock, gets `fixed`.
The Day stripes it, labels it with `fixed`, and gives it no handle and no edges.

**`duration` is a length.** When it says hours or minutes (`40 min`, `1 h 15`, `2 horas`) and
there is no `until`, it is how long the block lasts on the Day, and the block keeps that length
when the next one moves.

**A block may have an `id`.** Its `event` is then `{date}.{id}`, so an edit to it holds whatever
the pack does around it. An `id` is used once a day; two options may share one.

## Consequences

- An edit stored before this, on a block that now has an `id`, no longer finds its block.
- A `duration` still does not end the moment early: the next block or `until` does.
- `now` is the Day's clock time, and none on tomorrow; a host with no ruler can ignore it.
- e2e drags are pixels on the calendar's one scale, 1.6 dp a minute.

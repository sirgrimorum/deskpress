# 0035: the day like an agenda

Status: accepted, 2026-10-01. Amends 0013, 0025 and 0032.

## Context

0032 drags an hour to a new place in the order and leaves the fine steps to six buttons: fifteen
minutes earlier, later, shorter, longer, swap up, swap down. On the road that is several taps for
what an agenda does with one drag, and a stretch that runs into a booked train just overlaps it.

## Decision

**The page is the time axis.** Each hour stays as tall as it lasts. A finger's place reads as the
hour of the row under it, so a drag lands where it is dropped, in steps of fifteen minutes from
where the block was.

**Three drags.** The handle drags the whole block: same length, new start. On the picked block,
the top edge moves its start and the bottom edge its end. A block keeps fifteen minutes at least.

**A placeholder shows where it lands**, light green when it fits and light red when it does not;
dropped on red, nothing changes. An edge's preview tints the same way. The host asks the engine
once per step without storing anything (`would`, below), so the colour is the engine's answer.

**What never moves or shrinks:** a `locked` block, a block already over, one that ends on another
clock, the start of the one in progress, and the legs. A way of moving (`train`, `driving`,
`walking`, `flight`, `transfer`) moves but never shrinks. Nothing lands before now or leaves its
day.

**Lengths are kept.** A block lasts to its `until`, else to the next block (0032's `lasts`). When
a change needs room, in this order:

1. Free time: the gap between two blocks beyond the leg between them.
2. The blocks that touch are pushed along, each keeping its length.
3. At a wall (a locked block, the one in progress, midnight), room comes from shrinking: blocks
   with `type: free` first, then the others nearest the wall, none below half the length the pack
   gave it.
4. Still not enough: it does not fit. Red, and nothing moves.

**Shortening pulls the rest earlier.** A block that ends or moves earlier takes the blocks
touching it after it along, by the same minutes, up to the first free gap or wall. A free gap
stops it, so a stretch that fit in the free time and a shrink back leave the day as it was.

**Past a neighbour's middle, they trade order.** The hole the block leaves closes the way a
shortening does, and where it lands makes room the way a stretch does. The legs are worked out
for the new neighbours and fit the same way: a longer one pushes, a shorter one pulls.

**One edit, every fact at once.** `timeline.move {block, by}` and `timeline.resize {block, edge,
by}` (`edge` is `start` or `end`) work the day out on the drop and store each block they change
as its own hours, `plan.<event>: {at, until}`. They replace `.grow`, `.swap` and `.reorder`;
`.drop`, `.restore` and `.add` stay. A `shift` or `grow` stored before this still reads, on its
own block only.

**`would` answers without storing.** A fourth engine call, besides 0013's: the facts an action
would store, nothing kept and the screen's state untouched. Empty means it does not fit.

**The Day drops the step buttons.** `on_move` sends `{block, by}`, `on_resize` `{block, edge, by}`.
TalkBack gets the four fifteen-minute steps as actions on the picked block, the same edits a drag
sends. Built by hand in Compose: no library draws a day this way, and the rules live in the
engine, so the CLI and a later iOS shell answer the same.

## Consequences

- One drag replaces up to six buttons, and a stretch can no longer overlap a booked train.
- One edit can change several blocks; `restore` on a block brings back only that block.
- Facts are hours, not shifts: a newer pack that moves a block the person already moved keeps the
  person's hours.
- A block with no `until` gets one once it is resized or squeezed, and stops following the next.
- 0032's note on `grow` carrying in the pack's order goes away: there is no carry.
- Two phones editing one day at once can mix; each block's hours are their own fact (0034).

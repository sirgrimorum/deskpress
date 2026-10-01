# 0032: the day on one page

Status: accepted, 2026-09-30. Amends 0025. Amended by [0035](0035-the-day-like-an-agenda.md):
`timeline.move` and `.resize` replace `.reorder`, and the page is a time axis.

## Context

Phase 17: the day rearranged on one page, done when a day is reordered by hand and the travel
between follows. On the road the plan changes in the car: lunch moves before the museum, a stop
is dropped. Decision 0025 gave buttons for that, a block at a time, and nothing said how long it
takes to get from one place to the next, so a swap could leave twenty minutes to cross a city.

## Decision

**A leg is computed, never stored.** `timeline` puts one in the day's `blocks` before each timed
block whose `place` differs from the previous timed block's, when both places are in `places`, on
the same clock, and neither block is a way of moving (`train`, `driving`, `walking`, `flight`,
`transfer`). It ends at the block's hour, so it follows any move or swap and never a resize. It
has no `event`, no `place` and no `locked`: nothing can be done to it and the calendar skips it.
The agenda and the night screen show it with its `duration`.

**Its time is the pack's, else an estimate.** A place may carry `legs`, whole minutes to other
places, read both ways. With none, the straight line between the two `at` times 1.3, at the day's
`travel`: `walking` 75 m a minute, `transit` 333 plus ten to reach the stop, `driving` 1000 plus
ten to park. A day that says nothing walks up to 2 km of road and drives past that. Rounded up to
five minutes and marked `≈`. No network, no routing service: a guess that reads as one.

**`late` says the day does not fit.** A leg that has to start before the block it leaves is over is
`late`, drawn in the alert ink. The engine does not push the day around to make it fit; the person
does, by hand.

**Every timed entry gets `lasts`.** Minutes to its `until`, else to the next timed entry, else 60.
The page draws each row that tall; `day()`'s `block` and `next` carry it too.

**`timeline.reorder` moves an hour across the day.** `{block, to}`, `to` its new index among the
timed blocks that are not legs. It is a rotation: the ones it passes move by its length and it
takes the slot at `to`, all written as the `shift` of 0025. A locked block in the way, an index
out of range or the same one writes nothing.

**`Day` is the page.** The 18th component: `blocks` as rows as tall as they last, `picked`,
`on_tap` to pick one, and a handle labelled `move` that drags it; dropping sends `on_move` with
`{block, to}`. The tree version stays 4: an old renderer draws an unknown component as nothing.
The adjust screen draws it in place of the rows, and keeps 0025's buttons for the fine steps.

## Consequences

- Swapping lunch and the museum moves the travel between them with no extra fact stored.
- Estimates are rough on purpose: a pack that knows a leg writes it in `legs`.
- After a reorder, `grow` still carries in the pack's order, not the new one. Accepted: a resize
  after a reorder is rare, and its carry stops at the next locked block anyway.
- A set-off notice (`leave`) is not derived from the leg; the pack still writes it where it matters.

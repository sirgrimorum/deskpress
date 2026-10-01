# 0034: the trip phone to phone

Status: accepted, 2026-09-30.

## Context

Phase 10: two phones on one trip. A day rearranged, a task ticked, the car parked or a note
written on one should reach the other, with no server and nothing but text: no photos.

## Decision

**A trip is one text file.** `share` writes the pack's `id` and `updated`, every fact but the
phone's own (`holder`, `holder_until`, `returns_to`), when each was kept, and the pack's files
when it says when it was `updated`. The phone hands it to any app that sends a file; the other
phone picks it from the menu and is asked before anything is taken.

**Each fact, the later one wins.** The host stamps every fact it keeps, an action's or a sync's,
in milliseconds on the real clock, and keeps the stamps beside the facts. `take` keeps a fact sent
when it is not here, or differs and was kept later there. One sent with no stamp only fills a gap.

**A newer pack comes with it.** `pack.updated` is a date, or a date and time. The files come only
when the sent one is later, and only `.yaml`, `.yml` or `.json` files inside the pack, never a
hidden one; `pack.yaml` is written last. They are loaded before they are
written; one that does not load is not written, and the facts are taken anyway. A trip of another
pack is refused: it is opened first.

**`log` is the car and the notes.** A module with no content root: `plate`, `model`, `fuel`, `km`,
`spot` and `parked` from the facts `log.*`, and `notes`, the facts `note.<now.stamp>.<n>`, newest first.
The travel template keeps them on a log screen, a field kept only once it changed, and a note on a
screen of its own.

## Consequences

- Per fact, not per item: two phones adding to the same day's `added` list at once keep one list.
- Facts kept before this have no stamp: either phone's fills only a gap.
- Notes add up: one taken is never written over a note here; the same text in the same minute is
  skipped, and a clash takes the next free number in its minute.
- A note has no author and is never deleted; a car field cannot be emptied, only changed.
- A file written half way leaves a mix of packs; the next trip sent puts it right.
- A trip comes through a file the person picks, not a link that opens the app.

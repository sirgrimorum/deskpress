# 0021: calendar sync

Status: accepted, 2026-09-28.

## Context

Phase 7 part 4. The brief wants the plan in the phone's calendar, so its reminders ring and the
rest of the family sees it, without the app ever wiping an event somebody made by hand. The
engine knows the plan; only the host can reach the calendar.

## Decision

**The engine plans, the host writes.** `Engine::calendar(world, scope, known)` builds the events
of the timed blocks and compares them with `known`, the fingerprint per event id the host wrote
before. It answers `{add, change, remove}`. The host shows that, and writes it only on Apply.

**An event id is the block's place.** Each timed block carries `event`, `{date}.{list}.{n}`: the
list is `blocks`, `fixed` or `option-{id}`, `n` its index there. It stays while the block keeps its
place. `scope` is an event id, a date, or empty for the whole trip; an id is within it when it is
the scope or starts with the scope and a dot.

**What becomes an event.** The timed blocks of the holder, as the timeline shows them. An option's
blocks only once it is chosen: the recommended one is not written ahead. An event ends at `until`,
else at the next block, else an hour later. Its title is the text's first sentence, its notes the
text and the lines of the alerts set at its time, its location the place's `name` and new optional
`address`, and its reminder the earliest `notify_from` of those alerts.

**A picked account calendar and a ledger.** The first sync of a pack asks which writable calendar
to use, so the events sync to the account like any other. The host keeps a ledger per pack, the
calendar and a row and fingerprint per event id, in `files/calendar/<pack id>.tsv`. Only rows in
the ledger are changed or deleted. A row the person deleted by hand is written again on the next
change; a sync that fails part way keeps what it wrote in the ledger.

**Three buttons in the travel template.** The moment puts its block in (`block.event`), the agenda
the day (`day.date`) or the trip, the last two for adults only.

## Deviations from the design

- **An account calendar, not a trip calendar.** The spec wanted a calendar of the app's own.
  Picking one of the account's calendars lets the events reach every device of the account with
  no sync of our own; the ledger keeps the app to its own events.
- **Flights use the pack's timezone.** Every event is local to the pack's `timezone`; a flight
  that lands in another zone ends at the wrong hour. Fixed in [0023](0023-drift-review.md).
- **No opt-in for locators and PINs.** An event carries the block text, the place and the alert
  lines, never booking references. Nothing is sent that the plan does not already show.
- **No choice screen yet.** The travel template has no screen to answer a decision, so options
  reach the calendar only when a choice is stored some other way. Fixed in [0023](0023-drift-review.md).
- **Add and update are one action.** A sync writes whatever changed in its scope; there is no
  separate "update the calendar" button.
- **The schema's default radius** said 150 while the engine and the docs say 100; now 100.

## Consequences

- The FFI gains `Event`, `Plan` and `LoadedPack::calendar`. Timeline blocks gain `event`, which
  the template's Auto skips.
- The host command set adds `calendar.sync`, listed in [pack-format](../pack-format.md), and the
  app asks for the calendar permissions when a sync first runs.
- `make e2e` seeds a local calendar, `Deskpress e2e`, fresh each run, for the agenda and moment
  flows.

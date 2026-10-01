# 0023: the drift review of phase 7, what was fixed and what is kept on purpose

Status: accepted, 2026-09-28. Amended by
[0036](0036-a-pack-in-its-own-words-and-on-its-own-clocks.md): a last block with no `until` lasts an
hour, not the day, and decisions and alerts read on their own zone, else their day's.

## Context

Phase 7 ended with a pass over the brief, the spec, the design and the "Deviations from the
design" of every decision, looking for each place the work left them. Every drift found is either
fixed below or kept here on purpose, so none is left undecided.

## Fixed

- **A day with options asks.** The travel template has a `choose` screen: from the decision's
  `when` at `at`, an adult sees the question, the plan in force with why, and the others behind a
  button. Keeping a plan stores `choice.<date>` and writes that day to the calendar; while the decision
  is due, the agenda opens it again to change the plan.
- **A block is for somebody.** A block whose `for` does not name the person holding the phone is
  left out of the day.
- **Sheets.** The `sheets` module exposes every sheet as `{id, title, value}`; the agenda lists
  them and the `sheet` screen shows the whole value through `Auto`.
- **The kid guide.** The handoff stars the block's guide. A child the block names as guide sees
  "You could be the guide" and the place's `guide` script: facts to tell, a question with its
  answer behind a tap, and a challenge.
- **Night at seven.** The night screen starts at 19:00 on the day's own clock, not 21:00, once
  the last block is over, so a late dinner keeps its moment until its `until`; a last block with
  none lasts the day. A comparison with `day.time` sets the watch on that clock.
- **The days screen says why.** On a date the plan does not cover, a line says so above the days.
- **Per-day zones.** A day, a block and a block's `until` may name an IANA `zone`. Today, the
  block in force and the calendar events are read on that clock; the host passes the local time of
  each zone the pack names, so the engine still carries no zone rules.
- **Theme export.** When the picked folder can no longer be written, a valid theme edit offers
  "Save a copy", written where the person picks. This closes the export deviation of 0016.
- **The docs and schemas caught up.** The pack format no longer calls the definition a draft;
  `schema/pack.schema.json` covers `modules`, `derive`, `rules` and `screens`, and the content
  schema covers zones, a single-id `for` and a place's `guide`. The FFI README lists every export.
- **Coverage on the Android side.** Kover gates the code that does not draw (the view model, the
  pack files, facts, sync, calendar, location, theme) at the level measured when it was set, and
  `make android` runs it. What stays uncovered needs a device.

## Kept on purpose

- **Calendar (0021).** Events go to an account calendar the person picks, not a local one. No
  booking locator or PIN is written into an event, since a calendar is shared further than the
  phone. Adding and updating are one action: the plan says what it would do before it does it.
- **Documents (0018).** Insurance is three taps away, through the agenda. Nobody shows it in a
  hurry at a gate, so it does not earn a place on the moment.
- **The template (0014).** Cards come in the pack's order, the first card is not highlighted, and
  a block's type is never guessed from its text.
- **The renderer (0015).** The hero steps are the theme's, the `Row` time sits in its fixed
  column, and a folder is picked from the menu with no guided flow.
- **The theme editor (0016).** Type is edited by size only. The copy saved is the theme file,
  the one file the editor changes.
- **Kid mode (0017).** No quiz. The guide script's question is the one place a child is asked
  something, and its answer is shown, not scored.
- **Location (0020).** Geofences need the location permission, which the design does not draw;
  the spec wins. The permission is asked only when the pack has regions, and the position is
  used only while the app is open.
- **Fetching (0012, 0022).** A pack with no `sync` fetches nothing, as the brief says; one with a
  `sync` asks before its first fetch. Only `climate` syncs. A secret is `{name, param}`. There is no
  `fetch` host command (0022), and the CLI has no fetch command. Hosts are approved at the first fetch, not at load. Syncs run only while
  the app is open. `tag` stays. No e2e flow fetches, since a flow cannot rely on the network.
- **What a sync sends.** A query reads any name in scope, facts included, and the person approves
  a host, not what goes to it: the pack is theirs, so what it asks for is theirs to send.
- **Pinning.** A child's screen is pinned without device owner mode, so the system's own gesture
  leaves it. Unlocking asks for the phone's lock and lets through only a phone with none.
- **Zones.** A daylight saving change between now and a block is ignored: the offset is today's.
  A decision's `when` and `at` and an alert's stamps stay in the pack's timezone. Ordering an
  event's end against the next block compares local times across zones.

## The spec's extras

The spec names features the brief did not schedule. They are phases of their own now, the cheap
ones merged (see [roadmap.md](../roadmap.md)): packing lists, a font size setting, a navigator
choice, a "leave in five minutes" notice and moving or removing blocks at night; offline and
terminal maps with road content; a log with photos and sync between phones; questions to an
assistant, with menus and languages per person where a pack needs them.

## Consequences

- The travel brief is complete for phase 7, and each drift left has its reason here.
- The example pack gains the next day with two plans, so the `choose` screen has its flow.

# 0033: the trip around the days

Status: accepted, 2026-09-30.

## Context

Phase 19: what a trip needs beyond its days. A trip across six hours of zones has a body clock
to move, things to do before leaving, alerts that come back every day or are already dealt with,
and times that have to ring with the phone in a pocket and the app closed.

## Decision

**`jet_lag` is a root of days of steps.** Each day has a `date`, an optional `zone` and `steps`:
`{time, until, text, do, for, alarm}`. `do` is one of `wake`, `bed`, `sleep`, `light`, `dark`,
`coffee`, `no_coffee`; `for` filters by the holder like a block's. The module exposes today's day
in its own zone: its steps with a `state`, `now`, `next`, and on a bed or sleep step a `clash`,
the text of the first block of the plan still going or to come at that hour. A night, a lodging, a
flight, a train or a transfer is not a clash. The travel template shows the steps as a Body clock
group and the clash as an alert.

**`tasks` is a list to tick, not a plan.** `{id, title, deadline, who, status}`. A `status` of
`done` leaves it out; `open` and `partial` stay. A tick on the phone is the fact `task.<id>`, so it
can be undone; past its deadline and not ticked it is `late`. By deadline, the undated last, each
with `about` ("deadline · who") for the caption. The travel template opens them from a button
that counts the open ones.

**An alert may say it is over, repeat, and ring.** `status: done` hides it everywhere. `repeat:
{every, until}` shows it again every so many days to `until`, `notify_from` moving with it.
`alarm: true` asks for an alarm at its `notify_from`, else its `at` when it has an hour, else
its `at` at its `time`.

**Alarms are the engine's list and the phone's service.** `alarms(world)` returns each alarm
still to come, by time: alerts and jet-lag steps that ask for one, and a set-off notice for every
block with a `leave`, its words the pack's `ui.set_off_now`. The host sets them with the alarm
service on the real clock, never the one set by hand, after each load and each kept fact, and
sets them again after a restart. At most 100 are armed at once, soonest first; each one that
rings arms the next, so none is lost with the app closed. A notification with the alarm sound,
exact where the phone allows it. Only the open pack's ring. The CLI's `alarms` prints the same list.

**`Check` gets a `caption`.** A second line under its text, drawn muted when it is not empty.

## Consequences

- A clash is only seen where the plan says when a block ends: a block with no `until` runs to the
  next one, so a bed time inside a long evening shows only if a block begins after it.
- `next` stays within today: the body clock does not look at tomorrow's steps.
- Without exact alarms allowed, the system may ring a few minutes late. `USE_EXACT_ALARM` is for
  alarm apps on Play; an app shipped elsewhere is fine with it.
- An alert is resolved in the pack, not on the phone: there is no tap to mark one done.
- A task's deadline is a date, not a moment; `late` starts the day after.

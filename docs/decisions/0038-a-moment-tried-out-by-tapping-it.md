# 0038: a moment tried out by tapping it

Status: accepted, 2026-10-27. Amends 0030.

## Context

On the trip, walking the day ahead meant opening Settings, setting a clock and picking a place for
every block. The pretend strip said a moment was set, but not which one, and the way back to the
real clock was in Settings too.

## Decision

**A block is tried out with a tap.** A tap on a block in `agenda`, or on the bottom bar of
`moment`, runs `clock.try` with the block's `date`, `time` and `place`. The host sets the pretend
clock to that moment and the pretend place to that place's region, or none when it has no `at`.
From the agenda the template then goes home, so the moment of that block is what shows.

**`on_foot` on `Screen`.** It runs on a tap of the bottom bar. It is not a pill like the other
`on_<event>`.

**The strip says what is pretended and how to stop.** "Not now: <date> <time>, at <place>." when
the clock is moved, "Pretending: at <place>." when only the place is. A "Back to now" button next
to it puts the real clock and position back. Both are hidden for a child, as before.

## Consequences

- `clock.try` is a host command; the engine only names it, so `would` and the CLI do nothing with it.
- A host that does not know `clock.try` ignores it, and the tap does nothing.
- The e2e `pretend` flow walks two blocks by tapping and comes back to now.

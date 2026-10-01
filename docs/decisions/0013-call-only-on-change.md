# 0013: call the engine only when its answer can change

Status: accepted, 2026-09-27. Amends the calls in `architecture.md` and the screen tree. Amended
by [0035](0035-the-day-like-an-agenda.md): `would`, called while a drag is held, stores nothing.

## Context

The engine is pure: the same pack and the same world give the same tree. Calling it again with
nothing changed is wasted work, and redrawing a tree that did not change is flicker. But some
inputs move on their own: the clock always, the location often. Polling them would call the engine
all day for nothing, and the host cannot guess when a pack cares, because only the pack knows
which block starts at 11:15 or which place is a geofence.

## Decision

**Every tree comes with a `watch`: what would change it.**

```json
{ "watch": { "until": "2026-10-03T11:15:00+02:00", "regions": ["picasso", "hotel"] } }
```

- `until`: the next instant at which the answer changes. Each module reports its own next
  boundary (`timeline` the next block start or end, `alerts` the next `notify_from`, `choices`
  the next decision coming due, the day itself at midnight), and an expression that compares the
  clock records the instant it tested. The earliest future one wins; none means no timer.
- `regions`: the geofences whose crossing would change the answer. The host registers those and
  reports a crossing, never a raw position.

**The host calls `screen` only on an event**: the `until` timer firing, a region crossed, a
stored fact written, the holder changing, a sync arriving, or the app coming back to the
foreground (the clock may have passed `until` while asleep). A tap goes through `dispatch`, which
returns a tree and its `watch` the same way. Nothing else calls the engine.

**No flicker.** The previous tree stays on screen while the next one is computed; `Loading` is
shown only before the first tree. A tree equal to the current one is dropped (`StateFlow` skips
equal values and the UniFFI records compare by value), so an event that changes nothing redraws
nothing.

**No cache inside the engine.** One `screen` walks a few dozen values; the saving comes from not
calling it. A cache is added only if a measurement asks for one.

## Consequences

- One timer and a few geofences, instead of polling: better for the battery too.
- Every module that reads the clock or the location has to report its next boundary or its
  regions. A test per module checks that the tree at `until - 1s` equals the tree now and the tree
  at `until` does not.
- Built in phase 3 with the modules; the host side lands with the renderer in phase 5, and
  regions with location in phase 7.

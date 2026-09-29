# 0020: location

Status: accepted, 2026-09-28.

## Context

Phase 7 part 3. The places of a pack can carry `at`, a circle on the map. The brief wants three
things from it: a child holding the phone is called back when the device leaves a safe place;
the car is found where it was left; a place opens in a map app. Without a position, the plan's
schedule has to keep working as before.

## Decision

**Only while the app is open.** The host asks for the location permission when the pack has
regions, and follows the position only while the app is started. No background location, no
system geofences: the app is on screen when any of this matters.

**The host works out the regions, the engine decides.** `watch.regions` now carries each place's
circle, `{id, lat, lon, radius_m}` (radius 100 when the pack leaves it out). The host measures the
distance and passes `inside`, the smallest region first, and `located`, whether it has a position
at all. The engine is asked again only when `inside` changes, or on the first position.

**`here` and `away`.** `here` is the block's own place when the device is inside it, else the first
of `inside`, so a small place inside a larger one wins. `away` is true only when the device is
located, the block's place has `at`, and the device is out of it. Without a position `away` is
false, so the schedule rules as before.

**Leaving a safe place ends the turn.** The travel template's `safe` holds on the way (train,
driving, flight) whatever the position, and at the block's place (safe, a meal, the lodging) only
while the device is not `away`. Out of it, a child holding the phone gets the `complete` screen.

**Two host commands.** `{do: location.get, with: {then}}` runs `then` with the position as
`{lat, lon}`; with none yet the host says so, or asks for the permission. `{do: map.open, with:
{lat, lon, label}}` opens a map app, the only thing here that may reach the network, and only in
that app. The template saves the car on a parking block with `park`, stores it as `store.car`, and
the agenda offers `Find the car` from then on.

## Deviations from the design

- **The design said no permissions and no geofences.** The brief asks for the called-back child
  and the car, which need a position. The permission is asked only when the pack has regions,
  and the position is used only while the app is open.
- **The e2e flows pin the position.** Maestro's `setLocation` does not reach the emulator's GPS,
  so under a frozen clock the launch extra `at` stands in for it, as `now` does for the clock. The
  tracking itself is covered by the JVM tests of the pure parts, not by a flow.

## Consequences

- `World` gains `located`, and `Watch.regions` changes from ids to circles, in the engine, the
  FFI and the CLI (`--inside ''` says located, inside none). The tree version does not change.
- The host command set is now `device.unlock`, `phone.call`, `document.open`, `location.get` and
  `map.open`, listed in [pack-format](../pack-format.md).
- A position saved with `location.get` lives in the store like any other fact, on the device.

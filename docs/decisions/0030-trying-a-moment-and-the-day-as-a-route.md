# 0030: trying a moment out, answers that open, and the day as a route

Status: accepted, 2026-09-30.

## Context

Feedback from the first look on the emulator, two days before the trip. Testing a moment took the
`now` launch extra or the desk. An answer to a question sat in the row's caption, so the tap only
revealed something that was already there. "The day on a map" handed a map app one place, not the
day. And Settings offered map apps the phone may not have.

## Decision

**A pretend clock and a pretend place, as shell settings.** `Shell.shift` moves the clock by a
number of seconds and `Shell.at` pins the position as `lat,lon`. Both are kept in the shell's
preferences and are not pack facts. The clock runs on from the moment it was set, so the
engine's watch still fires; the shift adds to the e2e `now` as well. The place is picked from the
pack's regions, which are the places with an `at`, and while one is picked nothing is tracked. A
place only passed through is in the list when `places` gives it an `at`. A `road` entry has a time
and no position, so a stretch of road is tried out with the clock. While either setting is on, a
strip in the alert colour sits under the top bar on every screen, and tapping it opens Settings
(not in kid mode).

**Answers open in a `Dialog`.** It is a new component: `title` small, `text` large, over the
screen, and `close` for the button's label, with `on_close`. That makes seventeen components
([decision 0005](0005-screens-compose-components.md)). The template's `ask` screen lists the
questions only, and a tap opens the answer.

**The day as a route.** `chart` gains `route`: every stop in visit order as `{lat, lon, name}`,
with a stay repeated in a row dropped. `map.route` hands the stops to the host. The Android host
sends a Google Maps directions link (origin, waypoints, destination), the one kind of link that
carries several stops in order. The navigator named in Settings gets it first, and if that app
does not take it, the phone's own choice does. `map.open` stays as it is, for one place.

**Only the map apps on the phone.** Settings lists the apps that answer a `geo:` link, found
through a `<queries>` entry, plus "Ask". Sygic, Waze or any other app shows up when it is
installed and answers such a link.

## Consequences

- The tree version stays at 4. A renderer that does not know `Dialog` draws nothing for it; this
  host ships with the template that uses it.
- The e2e flows gain `pretend`. `ask` closes the dialog, `maps` checks the route button, and
  `settings` picks "Ask", because the emulator may carry no map app.
- A pack whose places have no `at` has an empty chart, route and pretend place list.
- Maps to look at inside the app, and dragging the day about, are phases 16 and 17.

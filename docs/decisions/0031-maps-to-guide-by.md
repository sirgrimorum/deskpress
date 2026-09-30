# 0031: maps to guide by

Status: accepted, 2026-09-30. Amends 0026 and 0020.

## Context

Phase 16: maps to look at inside the app, done when a park's points show on a real map with the
phone offline. The use it is for is a place, not the day: a citadel, a town, a museum, a square.
Somebody, often a child, picks where to go next and walks the family there, looking at the map
with no network.

Two things stand in the way. A place's `points` have no position, so there is nothing to put on a
map. And 0026 decided that nothing is fetched, so there are no tiles to put them on.

## Decision

**A point may carry `at`, `{lat, lon}`.** The same shape as a place's, with no radius. Points
without one still list as before; they are simply not on the map.

**The order of points is the pack's to give.** A guided museum has one, a citadel you wander does
not. A place with `in_order: true` joins its points in that order; without it they are pins with
no line between them. The day's chart is unchanged: it always joins its stops in visit order.

**`places` gains `area`.** The block's place's points that have an `at`, charted as `chart` is:
`{points, path, route, span_m}`, `path` and `route` empty unless the place is `in_order`. Null
when no point has an `at`. The map shows every point with a position, in kid mode too: a child
guiding needs all of them. The list under it stays filtered.

**Pins carry their position.** Every point and path step in `chart` and `area` gains `lat` and
`lon` next to `x` and `y`. The engine still never receives the device's position
([decision 0020](0020-location.md)).

**Tiles are kept on a press, and only then fetched.** Settings gains "Keep the trip's maps", for
an adult, with the size stated before it starts. It keeps vector tiles from OpenFreeMap (no key,
no cookies, no request limits) for every z12 cell that a place's circle touches, the circle padded
to a kilometre at least, at zooms 10 to 14. The OpenStreetMap tile servers forbid offline
download; OpenFreeMap forbids automated collection, and a few cells kept on a person's press is a
map client's cache, not that. Whole cells, about seven kilometres across, so the requests say
which city, not which street. About 1.5 MB a cell. Delete gives the space back. Everywhere else
the map library is kept offline: panning fetches nothing.

**The Map card opens a map to guide by.** When its pins carry positions, a tap on the card opens
it full screen on MapLibre: the kept tiles when the trip's maps are kept, plain paper when not, the
pins either way. The device's dot comes from the host's fixes, or the pretend place; its heading
from the phone's compass. A
pin picked from the row under the map draws a line from the dot to it and says how far. "Follow
me" turns the map with the phone, so the line points the way to walk. Opening it asks for the
position as a pack with regions does, and follows it closely while open (amends 0020).

**Indoors stays a `plan`.** A position is no use under a roof; the plan's pins in the picture's
own coordinates are.

## Consequences

- The validator checks a point's `at` as it checks a place's, and `in_order` as a yes or no. It
  warns about a point farther than the kept area reaches from its place, and about points with a
  position in a place with none, since no map is kept for them.
- The template draws `area` on the moment and the kid screen.
- The Android host gains MapLibre and a Settings section. The flows open the full-screen map on
  plain paper and never keep tiles; a keep is checked by hand, like the calendar and the folder.
- `places` gains `area`, one more known root, and the Map card gains its words for the full-screen
  map. The tree version stays at 4: an older renderer ignores the new fields.

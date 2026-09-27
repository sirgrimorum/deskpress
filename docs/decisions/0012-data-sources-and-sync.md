# 0012: where a module's data comes from, and the climate module

Status: accepted, 2026-09-27. Amends 0004 (a new module) and the offline rule in
`architecture.md` (the network is reachable, on the pack's terms).

## Context

Some data is known when the pack is written, like the weather a place usually has in October.
Some only exists later, like tomorrow's forecast. A pack has to be able to say which it wants,
and the app has to stay useful with no signal.

## Decision

**Every module that reads data can take it from two places, and says which in the pack.**

- `from`: a path in the pack's content. Offline, always there.
- `sync`: a source the app fetches from, and when: on a button, or automatically every so often.

```yaml
modules:
  climate:
    from: climate                  # the pack's data; leave out for sync only
    sync:
      trigger: auto                # button | auto
      every: 6h                    # auto only
      request:
        url: "https://api.example.org/forecast"
        query: {lat: place.lat, lon: place.lon}   # expressions
        secret: forecast_key       # optional: a name; the value is entered on the device
      read: {}                     # how the reply maps onto the module's shape
```

The two keys give the five modes. Neither is a load error.

| mode | `from` | `sync.trigger` |
| --- | --- | --- |
| 1. on the pack | yes | none |
| 2. pack, plus a sync button | yes | `button` |
| 3. sync button only | none | `button` |
| 4. pack, plus auto sync | yes | `auto`, with `every` |
| 5. auto sync only | none | `auto`, with `every` |

The rules:

- **The engine plans, the host fetches.** The engine builds the request from the pack and parses
  the reply; the host only performs a `fetch` command, like any other tool. HTTPS `GET` and a
  JSON reply only.
- **Synced data wins where it covers, the pack fills the rest.** A synced value replaces the pack's
  for the same key and date; everything else keeps the pack's value.
- **A sync never blanks anything.** The last good reply is stored on the device with its time.
  Offline, or when a fetch fails, the module keeps serving it and exposes its age (`as_of`) and
  the failure, so a screen can say "forecast from 9 hours ago".
- **The person approves the hosts.** When a pack with a `sync` loads, the app lists the hosts it
  will contact and asks once. A pack from someone else cannot quietly reach the network.
- **Secrets never live in the pack.** A pack names a secret; the person types the value on the
  device, and the host keeps it in the platform keystore. No secret appears in a screen tree.
- **Auto sync is the host's schedule**, only with network, never more often than the platform
  allows (15 minutes on Android).

**Data is not the world.** `from` and `sync` are about a module's data: what it knows, pulled
from the pack or a source. The world is what the host pushes in on every call: the clock, the
location, who holds the phone, the stored facts. Which of those a module reads belongs to the
module, not the pack: `timeline` reads the clock, `places` the location, `people` the holder,
`choices` the stored facts. That is what keeps the engine pure, and what lets the desk pass the
same world as arguments. A person correcting the world ("I am at the hotel") sets a stored fact,
which is already a world input.

**`climate` is a new module**, the first to use this. It answers "what is the weather here,
today", from the current day (`timeline`) and place (`places`), as `weather.high`, `weather.low`,
`weather.rain` (chance, in %), `weather.sunrise`, `weather.sunset`, `weather.summary` and
`weather.as_of`. Its content, for modes 1, 2 and 4:

```yaml
climate:
  units: metric                    # metric | imperial
  entries:
    - {place: barcelona, month: 10, high: 22, low: 15, rain: 30, sunrise: "07:45", sunset: "19:05",
       summary: "Mild; a light jacket at night"}
    - {place: barcelona, date: 2026-10-04, high: 25, summary: "Warm, the beach day"}
```

The most specific entry wins: a date over a month, a place over no place, and missing keys fall
through to the next match.

## Consequences

- Offline stays the default: a pack with no `sync` never touches the network, as before.
- The exact `read` mapping, the fetch command and the approval screen are built with the first
  real sync source, in phase 7, not before. This decision fixes the modes and the rules.
- Any later module (exchange rates, opening hours) gets the same five modes for free.

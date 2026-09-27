# 0004: Domain logic lives in built-in modules plus `derive`

Status: accepted, 2026-09-26.

## Context

Questions like "which day is today", "which block is current", "which place am I at" and "which
option applies" are subtle (timezones, empty times, option days, geofence overlap). If every pack
computed them with expressions, the language would grow list queries and every pack would get them
slightly wrong in its own way. If the shell hardcoded them for travel, the app would not be generic.

## Decision

The engine ships a small set of **modules**. A pack opts into each one and points it at its content.
A module reads its slice of content and the world, and exposes names to the expression language;
some also offer actions and ask the host for tools.

```yaml
modules:
  timeline: {entries: days, date: date, blocks: blocks}
  places:   {from: places, geofence: at}
  people:   {from: people, adult: adult}
  choices:  {}
derive:
  kid: not holder.adult
```

`derive` names further values from expressions, evaluated in order, each able to use the ones above.

Every module implements one interface: a config schema the validator checks, the names it exposes,
the actions it offers, and the host tools it needs. Adding a module never changes the core, which is
what keeps this extendable.

## Consequences

- The hard, shared logic is written and tested once. The first modules come from the first pack:
  `timeline`, `places`, `people`, `choices`, `alerts`, `documents`.
- A pack for a hospital stay or a sports season uses the same `timeline` pointed at other keys.
- A capability no module offers means writing a module, which is a release. That is intended: it
  is where correctness is hard.

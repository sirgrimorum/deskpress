# Templates

A template is a definition bundled with the engine: modules, derives, rules, screens and ui labels
that a kind of pack shares. A pack names one in its head and brings only its content, its theme
and what it changes.

```yaml
pack:
  id: my-trip
  name: My trip
  language: en
  timezone: Europe/Madrid
  content: content.yaml
  extends: travel
```

The templates live in `templates/<name>/pack.yaml`, and the engine carries them inside it, so a
pack never ships one and the app never reads one from disk. An unknown name is a load error that
lists the ones there are.

## How a pack overrides a template

| section | how it merges |
| --- | --- |
| `modules`, `derive`, `screens`, `ui` | by key: the pack's entry replaces the template's where it stands; a new key goes at the end |
| `rules` | the pack's rules are tried first, then the template's |
| anything else | the pack's replaces the template's |

A section of the wrong shape is kept as the pack wrote it, and the validator reports it. So a pack
in Spanish writes the `ui` labels it wants in Spanish; a pack with one screen of its own adds it
under `screens` and a rule that picks it under `rules`; a pack that wants a different moment
replaces `screens.moment` whole.

## travel

The trip app of the design: six views (moment, sheet, agenda, suggestion, complete, relay) and
three moments of the day with a screen of their own (morning, night, and the list of days).

**Rules**, first true wins:

| when | screen | what it answers |
| --- | --- | --- |
| `not day` | `days` | a date the pack does not cover: every day there is |
| `mine` | `suggestion` | the current block names the person holding the phone as `guide` or `for` |
| `kid and block` | `complete` | a child holds the phone through a block that is not theirs: give it back |
| `night` | `night` | from 21:00 with nothing left today: tomorrow's first hour |
| `not day.started` | `morning` | before the first timed block: the first hour and the day ahead |
| `block` | `moment` | inside a block: its answer, its place, what comes next |
| (none) | `agenda` | between blocks: the whole day, each block with its state |

**Screens and their actions:**

| screen | shows | actions |
| --- | --- | --- |
| `moment` | the day's critical alerts, the block's type, who holds the phone, the answer (`hero`), the weather, the place's parking and `during` keys, the block's own keys, high alerts, what comes next | `agenda`, `relay`, `points` and `ticket` (both open `sheet`) |
| `morning` | the first hour, the weather, the day's own keys, high alerts | `agenda`, `relay` |
| `night` | tomorrow's first hour and title, tomorrow's own keys | `agenda` |
| `agenda` | critical alerts, the weather, every block with its state, medium alerts as rows | `back`, `alerts` (opens `sheet`) |
| `days` | one row per day | |
| `sheet` | what `open` passed: a title, a value (a reference is followed), rows | `back` |
| `suggestion` | the block and its place's points, with what each has for children | `go` (opens `moment`) |
| `complete` | "time is up" and one button | `relay` |
| `relay` | one chip per person | `hold` stores `holder`, and the rules decide again; `back` |

**Derives:** `kid` (the holder is not an adult), `mine`, `night`, and `hero`, the answer at the top
of a moment: a drive or a walk its `duration`, a flight its `boarding`, a parking the place's
parking price or `ui.free`, a lodging its `check_in`, free time its `until`, anything else the
block's time. A block missing the key its type reads shows its time too, and the validator warns.

**What the pack gives it.** A block's `type`, `place`, `guide`, `for`, `until` and `ticket`; a
place's `during`, `parking` and `points` (points and ticket may also sit inside `during`); people with `adult`. Every other key of a block, a
place's `during` or `parking`, a day or tomorrow shows through `Auto`, by the unknown key rule, in
the pack's order. `conventions` decide which prefixes hide and which warn. The `ui` labels are in
English; a pack in another language overrides them by key.

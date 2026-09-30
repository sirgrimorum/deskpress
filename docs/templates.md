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
| `modules`, `derive`, `questions`, `screens`, `ui` | by key: the pack's entry replaces the template's where it stands; a new key goes at the end |
| `rules` | the pack's rules are tried first, then the template's |
| anything else | the pack's replaces the template's |

A section of the wrong shape is kept as the pack wrote it, and the validator reports it. So a pack
in Spanish writes the `ui` labels it wants in Spanish; a pack with one screen of its own adds it
under `screens` and a rule that picks it under `rules`; a pack that wants a different moment
replaces `screens.moment` whole.

## travel

The trip app of the design: six views (moment, sheet, agenda, suggestion, complete, relay),
three moments of the day with a screen of their own (morning, night, and the list of days), `kid`
for a child holding the phone, `documents` and `document` for what a counter asks for, and
`choose` for a day with options, and `ask` for what the pack can answer about the trip.

**Rules**, first true wins:

| when | screen | what it answers |
| --- | --- | --- |
| `kid and not safe and not extended` | `complete` | a child holds the phone where it is not safe and extra time ran out: alarm screen |
| `decision.due and not decision.answered and not kid` | `choose` | a decision whose hour has come and nobody answered: the recommended plan, the others behind a button |
| `not day` | `days` | a date the pack does not cover: every day there is |
| `mine` | `suggestion` | the current block names the person holding the phone as `guide` or `for` |
| `kid` | `kid` | a child holding the phone where it is safe: kid screen |
| `night` | `night` | from 19:00 on the day's own clock once the last block is over (one with no `until` lasts the day): tomorrow's first hour |
| `not day.started` | `morning` | before the first timed block: the first hour and the day ahead |
| `block` | `moment` | inside a block: its answer, its place, what comes next |
| (none) | `agenda` | between blocks: the whole day, each block with its state |

**Screens and their actions:**

| screen | shows | actions |
| --- | --- | --- |
| `moment` | the day's critical alerts, the block's type, who holds the phone, the answer (`hero`), the weather, the place's parking and `during` keys, the block's own keys, high alerts, the place's plan and its map of points (`area`); what comes next in the bottom bar | `agenda`, `relay`, `points` and `ticket` (both open `sheet`), `map` (the place in a map app, when it has `at`), `park` on a parking block (saves the position as `store.car`), `calendar` (the block into the calendar, `calendar.sync` with its `event`), `ask` (opens `ask`, while the pack has questions), `chart` (opens `chart`) |
| `morning` | the first hour, the weather, the day's own keys, high alerts | `agenda`, `relay` |
| `night` | tomorrow's first hour, its own keys; its title in the bottom bar | `agenda`, `adjust` (opens `adjust` on tomorrow) |
| `agenda` | critical alerts, the weather and why its last sync failed, every block with its state, medium alerts as rows | `back`, `alerts` (opens `sheet`), `documents` (opens `documents`), `car` (where the car was left, in a map app, once saved), `calendar_day` and `calendar_trip` (the day or every day into the calendar; not for a child), `weather` (`climate.sync`, when the module syncs; not for a child), `sheet` (one row per sheet, opens `sheet`), `choose` (opens `choose` to change a kept plan while its decision is due; not for a child), `ask` (opens `ask`, while the pack has questions), `chart` (opens `chart`), `adjust` (opens `adjust`) |
| `days` | a line saying the plan does not cover today, then one row per day | |
| `sheet` | what `open` passed: a title, checkable rows when it passed `ticks`, a value through `Auto` (a reference is followed), rows | `back`, `tick` (stores the row's own fact) |
| `chart` | the day's places drawn on plain paper, the path between them | `back`, `route` (every stop in order to a map app, `map.route`, with two stops or more), `map` (the place in a map app, when it has `at`) |
| `adjust` | every block of the day, or of tomorrow when `open` passed `next`; the picked one with the buttons that move it | `back`, `pick`, `earlier`, `later`, `shorter`, `longer`, `up`, `down`, `drop`, `restore` (all `timeline.*`), `add` (opens `add`) |
| `add` | a block of your own: an hour and what it is | `back`, `set_time`, `set_what`, `keep` (`timeline.add`, then back) |
| `choose` | the decision's question, a line saying the day follows the recommended plan until chosen (only before one is), the option in force with why and its own keys | `agenda` (Today), `others` (shows every option), `pick`, `confirm` (stores `choice.<date>`, runs `calendar.sync` for that date and goes home) |
| `documents` | a group per person with their documents, then the ones that are nobody's. None for a child | `back`, `open` (opens `document` with the row's document) |
| `document` | whose it is, a button per number to call, its fields, and the button to the file | `back`, `call` (`phone.call`), `file` (`document.open`) |
| `suggestion` | the block and its place's points, with what each has for children | `go` (opens `kid` if kid, else `moment`) |
| `kid` | child holding the phone: the place's map when its points have `at`, points for kids, what comes next, no menu, give back button; when the block names them its guide, "You could be the guide" and the place's `guide` script | `give_back` (unlocks via host, then `returned`), `returned`, `answer` (shows the script's answer), `ask` (opens `ask`, while the pack has questions) |
| `complete` | alarm screen when time is up or unsafe: big "Time is up", give back and 15 more minutes | `give_back` (unlocks then `returned`), `more_time` (unlocks then `extend`), `returned`, `extend` |
| `ask` | the questions the pack can answer, each opening its answer in a `Dialog`; on a phone with a model of its own, a field to type in and the model's words in a card | `back`, `show` (opens the tapped question's answer), `close`, `type`, `say` (`assistant.ask` with the typed question and the day as facts), `answered` |
| `relay` | one chip per person (children offered only if safe or adult), a star on the block's guide | `hold` stores `holder`, `returns_to` and clears `holder_until`; `back` |

**Questions:** what is happening now, what is next, where we are going, what we are doing tomorrow,
the weather, and what papers we have. Each is offered only while its data is there; `where` and
`tomorrow` are also offered outside the app.

**Derives:** `kid` (the holder is not an adult), `safe` (here is safe, or a train/driving/flight block, or the block's place is safe or a meal or lodging and the device is not `away` from it), `extended` (`now.stamp < store.holder_until`), `mine`, `guiding` (the block names the holder its guide), `night`, and `hero`, the answer at the top
of a moment: a drive or a walk its `duration`, a flight its `boarding`, a parking the place's
parking price or `ui.free`, a lodging its `check_in`, free time its `until`, anything else the
block's time. A block missing the key its type reads shows its time too, and the validator warns.

**What the pack gives it.** A block's `type`, `place`, `guide`, `for`, `until`, `ticket` and
`road`; a place's `during`, `parking`, `points` (each with an optional `at`), `in_order`, `guide` and `plan`
(points and ticket may also sit inside `during`); people with `adult`. Every other key of a block, a
place's `during` or `parking`, a day or tomorrow shows through `Auto`, by the unknown key rule, in
the pack's order. `conventions` decide which prefixes hide and which warn. The `ui` labels are in
English; a pack in another language overrides them by key.

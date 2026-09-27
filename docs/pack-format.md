# Pack format

A pack is a folder of three parts. The shell loads it, validates it, and becomes the app it
describes.

```mermaid
flowchart TD
  root["my-pack/"] --> manifest["pack.yaml: the app definition. Name, modules, rules, screens"]
  root --> content["content.yaml: the data the app shows"]
  root --> theme["theme.yaml, optional: the design system"]
  root --> files["files/, optional: PDFs and images the content points at"]
```

This page has two halves. **The definition** (modules, rules, screens, expressions) is new and is a
draft until the engine that runs it lands; it follows decisions 0002 to 0005. **The content** (days,
places, people, alerts, documents, sheets) is what the first modules read, and is stable, except
`climate`, which is new.

Two rules before the keys.

**Canonical keys are English; values are free.** The shell reads `days`, `blocks`, `places`,
`people`. What those keys hold can be in any language and any script, and the shell never parses
your prose: it places it. If your file already uses key names in another language, do not rewrite
it; declare a `keymap` in the manifest. The keymap is grouped by context (`day`, `option`, `place`,
`point`, `alert`, and so on), because the same canonical key has different names in different
places: `why` is `por_que` in an option and `por_que_importa` in a point.

**Unknown keys render, they do not break.** Any key the shell does not know becomes a card labelled
with the key name, underscores turned into spaces, holding its value. That is the feature that lets
you enrich a pack tonight and see it on the phone tomorrow without anyone writing code.

## pack.yaml

```yaml
pack:
  id: ruta-2027               # slug. Stable forever: calendar event ids derive from it
  name: Mi ruta               # what the app calls itself once this pack is loaded
  language: es                # BCP 47. Picks the shell's own handful of labels
  timezone: Europe/Lisbon     # IANA. The moment is computed in this zone
  content: content.yaml       # one file, a list merged in order, or a root to file map
  theme: theme.yaml           # optional. Without it the shell uses its default theme
  files: files/               # optional. Base for every file reference

conventions:                  # optional, all of it
  alert_prefixes: [warn, alert]   # keys starting with these render as an Alert, not a Card
  hidden_prefixes: [source]       # keys that carry provenance and never reach a screen

keymap:                       # optional. canonical key -> the name this pack uses, by context
  root:
    days: itinerario.dias     # under root, a dotted path is allowed
    places: lugares
    people: viajeros
  day: {date: fecha, title: titulo, blocks: bloques, fixed: fijo_del_dia, options: opciones}
  point: {name: nombre, what: que_es, why: por_que_importa, for_kids: para_los_ninos}
  values:
    severity: {critical: critica, high: alta, medium: media, low: baja}

ui:                           # optional. overrides the shell's own labels
  today: Hoy
  back: Volver
  next: Lo que sigue
  open_map: Abrir en el mapa
```

**`content` takes three forms.** One file name. A list of file names, merged in order, which is
refused when two of them define the same root key, because the second one would quietly win. Or a
map from root key to the file that holds it, which is the form to use once a pack is split: each
file is rooted under the key that names it, so two files cannot collide.

```yaml
pack:
  content:
    days: days.yaml           # days.yaml holds the list itself, with no "days:" line above it
    places: places.yaml
```

Those keys go through `keymap.root` like any other, so a pack that calls its days something else
names them that way here too.

## The definition

Draft. The rest of `pack.yaml` says how the app behaves: which modules read the content, which
screen shows when, and what can happen on each screen. A pack may also start from a template with
`pack.extends: travel` and only override what differs.

```yaml
modules:                      # shared logic the pack opts into, pointed at its content
  timeline: {entries: days, date: date, blocks: blocks}
  places:   {from: places, geofence: at}
  people:   {from: people, adult: adult}
  choices:  {}

derive:                       # further names, in order, each from an expression
  kid: not holder.adult

rules:                        # the main machine: first true `when` picks the screen
  - when: decision.due and not decision.answered
    screen: choose_day
  - when: block.locked and block.missed
    screen: blocker
  - screen: moment            # the last rule has no `when`

screens:                      # each screen is its own machine
  choose_day:
    state: {picked: null}     # local, reset when the screen leaves the top
    actions:
      pick:    [{set: picked, to: $arg}]
      confirm:
        - {if: picked, store: "choice.{day.date}", value: picked}
        - calendar.sync
    layout:
      - BigValue: {text: decision.question}
      - Segmented: {items: day.options, on_tap: pick}
      - Button: {label: ui.confirm, on_tap: confirm}
```

### Where a module's data comes from

A module reads the pack's content (`from`), a source it syncs (`sync`), or both. Synced data wins
where it covers and the pack fills the rest; a failed sync keeps the last good data and its age.

```yaml
modules:
  climate:
    from: climate
    sync: {trigger: button, request: {url: "https://api.example.org/forecast"}}
```

| mode | `from` | `sync.trigger` |
| --- | --- | --- |
| on the pack | yes | none |
| pack, plus a sync button | yes | `button` |
| sync button only | none | `button` |
| pack, plus auto sync | yes | `auto`, with `every: 6h` |
| auto sync only | none | `auto`, with `every` |

A module with neither is a load error. The person approves the hosts once, and a `secret` is
only a name: its value is typed on the device. The request and reply mapping are a draft until
the first sync source lands; see [decision 0012](decisions/0012-data-sources-and-sync.md).

### Rules

An ordered list. Each rule has a `screen` and, except the last, a `when`. The engine re-runs the
list whenever an input changes. When the chosen screen's name changes, the navigation stack is
cleared: the situation outranks what the person was reading.

### Screens

| key | what it is |
| --- | --- |
| `state` | local values with their starting value. Reset when the screen leaves the top of the stack |
| `actions` | named lists of effects, run in order |
| `layout` | a list of components, each bound to expressions and actions |

Effects:

| effect | what it does |
| --- | --- |
| `{set: name, to: expr}` | change a local state value |
| `{store: key, value: expr}` | persist a fact on the device. Stored facts are inputs, so this can change the screen |
| `{open: screen, with: {name: expr}}` | push a screen, passing values it reads as `params` |
| `back`, `home` | pop one screen, or clear the stack |
| `module.action` | a module's action, like `calendar.sync` or `map.open`. May become a host command |

Any effect takes `if: expr` and is skipped when it is false. `$arg` is the value the component sent.

### Components

The closed set a layout can use. Every prop takes an expression; colors, sizes and fonts come from
the theme and cannot be set here.

| component | what it draws |
| --- | --- |
| `BigValue` | the answer, at the top, large |
| `Label` | a line of small text |
| `Card` | a titled box with a body; `kid: true` in kid mode becomes a kid box |
| `Row` | label and value on one line |
| `PhraseRow` | a phrase, its translation and a hint |
| `Alert` | a warning, by severity |
| `Chip` | a small tag |
| `Button` | an action |
| `Segmented` | a choice between a few items |
| `Missing` | a fact nobody confirmed, drawn as a striped hole |
| `Auto` | expands any value by the unknown key rule: cards, alerts, rows |
| `Screen` | the frame: title, back link, the way to the first screen |

`each: expr` on any component repeats it once per item, with the item available as `item`.
`if: expr` hides it when false. The set grows only by shell release; see decision 0005.

### Expressions

Used by `when`, `if`, `derive`, every prop and every effect value. Parsed at load, never run as code.

```
expr     := or
or       := and ("or" and)*
and      := not ("and" not)*
not      := "not" not | compare
compare  := value (("==" | "!=" | "<" | "<=" | ">" | ">=" | "in") value)?
value    := literal | path | call | "(" expr ")"
path     := name ("." name)*
call     := name "(" expr ")"
literal  := number | "'" text "'" | true | false | null
```

Functions take one argument. `count` is the length of a list, a mapping or a text; `empty` is
whether that length is zero; `first` and `last` are the ends of a list, and null for anything else.
A name or key that is not there is null. An expression is at most 128 tokens; past that, split it
with `derive`.

A text prop may be a template instead: every `{expr}` inside it is replaced by its value, so
`"{block.time} · {place.name}"` is text, not an expression.

Names an expression can read: what the modules expose, what `derive` defines, the screen's `state`
and `params`, `content` for the raw data, `ui` for the shell's labels, and `store` for stored facts.
Anything else is a load error with the path and the column.

## days

From here on, the content: what the first modules read. `days` is what `timeline` reads, `places`
what `places` reads, and so on. The spine of a travel pack. One entry per day, and inside it, blocks of time.

```yaml
days:
  - date: 2026-04-11                  # required, YYYY-MM-DD
    title: Arrival and the tile museum  # required
    who: [rita, tomas]                # optional, person ids
    sleeps_at: Casa da Graca          # optional, any free key is allowed here too
    blocks:
      - ["09:20", "Land at LIS T1. Passport queue, 45 to 90 min to clear."]
      - ["15:30", "The market by the river, closed on Mondays."]
      - ["", "Long flight behind you: nothing demanding."]
```

A block is a list: **time, text, and an optional map**. A block whose time is empty is a note about
the whole day: it shows up in the agenda and never becomes a screen of its own.

The third element is what turns a line of text into a real moment:

```yaml
      - ["11:15", "Museu Nacional do Azulejo, top floor first",
         {type: visit, place: azulejo, for: [rita, tomas]}]
      - ["18:40", "Ferry from Cais do Sodre",
         {type: transfer, place: cais_do_sodre, guide: tomas, locked: true}]
```

| key | what it does | if missing |
| --- | --- | --- |
| `type` | picks which cards the travel moment screen builds | guessed from the text, and if that fails the moment is time plus text |
| `place` | id in `places`, where `during` and `parking` come from | the screen keeps the block's own text |
| `for` | person ids this block belongs to | everyone |
| `guide` | who is offered the chance to present this moment | nobody, and the kid screen does not appear |
| `until` | closes the moment before the next block starts | the next block closes it |
| `locked` | an hour that cannot move: a booked train, a timed entry | the hour is treated as soft |
| `language` | which branch of your phrase sheets applies here | no phrase sheet |

### The thirteen types

`visit`, `train`, `driving`, `walking`, `meal`, `event`, `flight`, `parking`, `lodging`, `night`,
`morning`, `transfer`, `free`. Thirteen values, not fourteen. They belong to the
travel template: each one decides which cards its moment screen assembles and nothing else. Adding
a type is a change to the template, so prefer an existing one plus your own free keys.

### A day with options

Some days are not one plan. A day can carry blocks that happen regardless and two or more closed
alternatives, and the shell keeps the choice on the device.

```yaml
  - date: 2026-04-12
    title: The coast or the hill town
    fixed:                                  # happen whatever gets chosen
      - ["08:30", "Pick up the car."]
      - ["20:00", "Dinner, both of you."]
    options:
      - id: coast
        name: The coast and an early arrival
        recommended: true
        cost: lunch and the parking
        why: three driving days follow; arriving early is what makes them work
        consequence: the hill town moves to the 15th
        blocks:
          - ["09:30", "Leave the city on the A5."]
      - id: hill_town
        name: The hill town
        cost: 24 EUR for the two funiculars
        blocks:
          - ["09:00", "Train from Rossio."]
    decision:
      when: 2026-04-09          # the night the app asks, once
      at: "21:00"
      decides: [2026-04-15]     # other days whose options depend on this answer
```

Rules the shell applies, and they are the whole feature:

- Until somebody chooses, the app behaves as the `recommended` option and says so in one line.
- `fixed` blocks are the only ones written to the calendar up front. The chosen option's blocks are
  written when it is chosen, and the previous option's events are removed. They are the only
  calendar events the app ever retracts.
- A day listed in `decides` filters its own options against the answer: an option whose `id`
  matches a `requires` on that day stays, the rest are dropped. Declare that with
  `requires: {date: 2026-04-12, option: hill_town}` on the dependent option.
- The choice lives on the device, never in the pack. The pack is a document, not a state file.

## places

```yaml
places:
  azulejo:
    name: Museu Nacional do Azulejo
    kind: museum                      # free prose. Never used to choose a view
    at: {lat: 38.7248, lon: -9.1139, radius_m: 120}   # optional, enables geofences
    safe: true                        # optional, a kid session may run here
    during:                           # the content of a moment at this place
      type: visit
      hours: "10:00 to 18:00, closed on Mondays"
      ticket: bookings.azulejo        # a path reference, resolved before the view sees it
      toilets: "Ground floor, past the shop"
      warn_bags: "Backpacks go in the lockers, 1 EUR coin, returned"
    parking:
      where: "On the street below the cloister"
      price: "[to confirm]"
      verified: 2026-03-02
    points:                           # ordered. The route is the order. May also sit inside during
      - id: panorama
        name: The great panorama
        what: "Twenty three metres of the city as it looked before the earthquake"
        why: "It is the only picture of the streets that are gone"
        for_kids: "Find the boat with three masts. There are four of them"
```

`points` is where kid mode earns its keep: in kid mode the sheet is **filtered**, not translated.
Only points with `for_kids` appear, and that text is what shows. A point without it is not a gap to
fill: there was nothing there to offer them.

`verified` never renders as a card. It is the date somebody checked the fact, and the screen uses it
for one thing: if the fact is older than a month, the card carries that date in small type.

## people

```yaml
people:
  - {id: rita,  name: Rita,  adult: true,  theme: rita}
  - {id: tomas, name: Tomas, adult: false, theme: tomas}
```

`adult` decides the mode, and it is not a setting anyone can flip. Who is holding the phone is
chosen once and changed on the handoff screen.

## alerts

The only source of the alert component in the whole app. Nothing is computed: alerts are read,
filtered by time and sorted by severity.

```yaml
alerts:
  - id: ferry_last_connection
    severity: critical               # critical | high | medium | low
    at: 2026-04-11T18:40             # when the thing happens. A date alone is fine
    time: "18:40"                    # optional, when at is only a date
    notify_from: 2026-04-11T17:30    # when it starts showing
    title: "The 19:10 ferry does not connect"
    detail: "It arrives after the last bus on the other side. Budget an hour."
    action: "Leave the museum cafe by 18:10"
    see: bookings.azulejo            # optional path reference
```

| severity | where it shows |
| --- | --- |
| `critical` | above the answer, in the moment and in the agenda |
| `high` | below the moment's cards |
| `medium` | a row in the day's agenda, not in the moment |
| `low` | only in the alerts sheet |

## documents

Files that have to open with no signal, because somebody at a counter is asking for them.

```yaml
documents:
  - id: insurance_rita
    title: Travel insurance
    for: rita                        # person id. One card per person
    file: files/insurance-rita.pdf
    fields:
      Certificate: "PT-2210554"
      Cover: "20,000 EUR medical, repatriation included"
      Call: "[to confirm]"
      Valid: "6 h before departure until 6 h after the return flight"
```

The shell renders one card per document, groups them by `for`, opens the file full screen at maximum
brightness, and never needs the network to do it.

## climate

Draft, read by the `climate` module. The weather a place usually has, and any day you know better.

```yaml
climate:
  units: metric                      # metric | imperial
  entries:
    - {place: lisbon, month: 4, high: 20, low: 12, rain: 35, sunrise: "06:55", sunset: "20:10",
       summary: "Spring; showers pass quickly"}
    - {place: lisbon, date: 2026-04-11, high: 23, summary: "Warm for April"}
```

| key | what it is |
| --- | --- |
| `place` | id in `places`. Missing: the whole pack |
| `month` or `date` | a month (1 to 12), or a day. A date beats a month |
| `high`, `low` | temperatures, in the pack's units |
| `rain` | chance of rain, in % |
| `sunrise`, `sunset` | `HH:MM`, local time |
| `summary` | one line a person reads |

The most specific entry wins, and a key it lacks falls through to the next match. The module
exposes the result for the current day and place as `weather.high`, `weather.low`, `weather.rain`,
`weather.sunrise`, `weather.sunset`, `weather.summary` and `weather.as_of`.

## sheets

Everything else. Any tree under `sheets` becomes a sheet: a title, a back link to the moment that
opened it, and rows or cards. This is where bookings, phrase lists, budgets, packing lists and
contacts live, and none of them needs shell support.

If there is no `sheets` key at all, **every root branch the shell does not know becomes a sheet**.
A pack that was already a tree of your own top level sections needs no rearranging for this: the
unknown key rule applies to the root as well.

```yaml
sheets:
  phrases:
    portuguese:
      - {there: "Dois bilhetes, por favor", here: "Two tickets, please", zone: Lisbon}
  bookings:
    - id: azulejo
      what: "Museum, one adult one child"
      code: "MNA-20418"
      when: 2026-04-11T11:15
```

## Path references

A value that looks like `bookings.azulejo` and resolves inside the pack is a reference; the
reader resolves it before any view sees it. If it does not resolve, it is text and shows as text.
The pattern is `^[a-z_][a-z0-9_]*(\.[a-z0-9_]+)+$`.

Resolution walks three shapes, because packs use all three:

- a map walks by key: `places.azulejo.parking.price`;
- a list of records walks by `id`: `bookings.azulejo` is the record whose `id` is that;
- `days` walks by `date`.

## The unknown key rule, exactly

Given a key the shell does not know, inside `during`, `parking`, a place, a day or a sheet:

1. If it starts with one of `conventions.hidden_prefixes`, it does not render at all.
2. If it starts with one of `conventions.alert_prefixes` (`warn`, `alert` by default), it renders as
   an Alert.
3. If its name ends in `__YYYY_MM_DD`, it renders only on that date, as an Alert. Use this for the
   note that matters on one day and is noise on the other twelve.
4. Otherwise it renders as a Card whose label is the key with underscores turned into spaces, and
   whose body is the value. A list becomes rows; a map becomes a small sheet.

`[to confirm]` anywhere in a value renders as a striped hole with the words inside. A missing price
shows as missing on purpose: an invented one is worse than a hole.

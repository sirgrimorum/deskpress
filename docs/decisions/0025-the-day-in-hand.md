# 0025: the day in hand

Status: accepted, 2026-09-29.

## Context

Phase 8, the spec's small extras around the day itself: packing lists you can tick off, a text
size, which map app opens, a notice telling you when to set off, and moving, resizing, removing or
adding a block on the device. Four of those write something the pack never said, and the pack is a
document: nothing here may be written back into it.

## Decision

**Every edit is a stored fact, keyed by the block's `event` id.** `plan.<event>` holds `{shift,
grow}` in whole minutes, or `{off: true}` for a block taken out; `added.<date>` is a list of
`{time, text}`, whose blocks get `event` ids `<date>.added.<n>` and behave like any other block
from then on. The timeline applies them every time it builds the day, so the pack file is never
touched and clearing the facts restores the pack exactly.

**A resize moves what follows it; a locked hour stops the push.** `grow` on one block carries to
every later block, `shift` moves only the block it names. A block with `locked: true` never moves
and ends the carry, so a booked train stays put and the hours after it keep what the pack gave
them. An edit is clamped to twelve hours either way, and a moved time never leaves its own day.

**`timeline.*` actions are answered by the engine, not the host.** `timeline.move`, `.grow`,
`.swap`, `.drop`, `.restore` and `.add` change stored facts and touch nothing outside, so
`dispatch` folds them into the store patch and emits no command. Arguments that make no sense
write nothing, and never fall through to a device tool.

**A sheet opts in to being a list.** `check: true` on a sheet turns its `items` into rows to tick
off. The module exposes each row as `{fact, text, done}`, where `fact` is the flat store key
`tick.<sheet>.<n>`, and keeps `check` and `items` out of `value` so the unknown key rule does not
draw them twice. The pack ticks a row with `{store: "{$arg}", value: "not at(store, $arg)"}`: one
store, no arithmetic and no string building in the grammar.

**`leave` says how long before its time you have to set off.** A whole number of minutes on a
block. When that block is `next`, the timeline adds `leave_at`, the local hour to go, and
`leaving`, the guard, and pushes the instant onto the watch. The notice costs one timer, not a
redraw every minute.

**Two new components, and two settings that are the shell's, not the pack's.** `Check` is a line
to tick off, `Field` is a line to type on: fifteen components now. The text size scales every type
step over whatever the theme asked for, and the map app is a package the `map.open` intent is sent
to, falling back to the phone's own choice. Both live in the shell preferences, so they hold across
packs.

## Deviations from the design

- **The time picker is gone.** The design had one picker per block. Moving, growing and swapping in
  fifteen minute steps rearranges the whole day instead, which is what the day in hand actually
  needs, and it never produces an hour the pack's own order disagrees with.
- **The adjust screen reads the day live.** A screen's `params` are captured when it opens, so an
  edit made on it would not show. It carries only a boolean `next` and reads `day` or `tomorrow`
  itself; its only local state is `picked`, an `event` id, which no edit changes.

## Consequences

- The engine grows `at(value, key)` in the grammar, `Check` and `Field` in the closed component
  set, `leave` in the block validator, and the `plan`, `added` and `tick` fact families. The tree
  version does not change.
- `modules/plan.rs` is the one place that rewrites a day, and `timeline` is the one module whose
  actions the engine answers itself.
- The travel template gains the `adjust` and `add` screens, the set-off notice on `moment` and
  `morning`, and the way into rearranging from `agenda` and `night`.

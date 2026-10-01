# 0036: a pack in its own words and on its own clocks

Status: accepted, 2026-10-01. Amends 0014, 0023 and 0033.

## Context

Packs written in another language, for a trip that starts on another continent, found the
kit's gaps: an alert or a decision read on the pack's clock while the people were on another,
English words on a screen in another language, a night that never came, and sheets titled by
their raw key.

## Decision

**The host passes the clocks, the engine never computes one.** `world.zones` maps a zone to its
local time. A block, an alert, a task's deadline and a decision read on their own `zone`, else
on the zone of their day, else on the pack's. An alert takes `for` and `until` like a block. The
CLI takes each clock as `--zone <zone>=<time>` and says on stderr which ones it was not given.

**The pack brings its words; the validator names what is left.** The template's labels are
English. A pack in another language overrides them in `ui`, names its block types in `ui.types`
and writes its own `questions`. When it extends a template, one warning at `ui` lists every
label, type and question still in English, so they can be fixed in one pass. No translations
ship with the engine. Known keys that leaked through `Auto` (`cost`, `consequence`, `where`) get
their own labelled cards; `who` and `verified` are never drawn.

**A last block with no `until` ends.** It lasts as long as the day page draws it, an hour, and
then the night comes. A block in the middle still lasts until the next one.

**Smaller fixes, same rule: nothing on the screen the pack did not say.**

- A sheet is titled by its key with a capital first letter; a key under `hidden_prefixes` is no
  sheet. `climate.units` and `climate.entries` rename through the keymap.
- An option with no `why` shows its name; the choose screen says "until chosen" only while a
  recommended option is left.
- The days screen shows the alerts and the body clock before the trip starts.
- A kid turn with no end says "Not safe here", not "Time is up", and names whose turn it is.
- The YAML reader takes a flow mapping on the next line. The validator flags a reference to another
  reference, which shows as text.
- `deskpress --version` prints the version and the screen tree version.

## Consequences

- A pack on two clocks needs the host's local times; without them it reads on the pack's clock,
  and the CLI says so.
- A Spanish pack that extends `travel` loads with one warning until its labels are written.
- A screen that relied on a last block lasting the whole day now shows the night after an hour.

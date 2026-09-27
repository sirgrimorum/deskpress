# Authoring a pack

You need a folder, one YAML file and about an hour. `docs/pack-format.md` is the reference for every
key; this page is how to get from a pile of notes to something that works on the phone.

## Start from what you already have

Do not start from the schema. Start from the notes you already keep: the confirmation emails, the
list in your phone, the spreadsheet, the messages where you agreed on something. A pack is those
notes arranged by **when you will need them**, and that is the only transformation that matters.

The question to ask of every line: **at what moment does this matter?** A booking code matters at a
counter on one morning. A phrase matters when you are ordering. A hotel PIN matters at 23:00 on the
day you arrive. Put each thing where that moment can reach it, and delete anything with no moment,
because it was never going to be read.

## The shortest pack that works

```yaml
# pack.yaml
pack:
  id: my-week
  name: My week
  language: en
  timezone: Europe/Madrid
  content: content.yaml
```

```yaml
# content.yaml
days:
  - date: 2026-10-03
    title: First day
    blocks:
      - ["09:00", "Pick up the keys at the office on the corner."]
      - ["13:30", "Lunch, the place with the blue awning."]
```

That loads and it works. Everything else is enrichment, and you can add it later without touching
what already runs.

## Then add, in this order

1. **Places**, for anywhere you go more than once or anywhere you need something on arrival. Attach
   them to blocks with `place:`. This is what turns a line of text into a screen with content.
2. **Coordinates**, `at: {lat, lon, radius_m}`, on the places where being there changes the answer.
   A radius of 100 to 150 m is right for a building, 400 m for a neighborhood, 1 km for a town.
3. **Alerts**, for the handful of things that are expensive to get wrong. Keep this list short. Four
   real alerts get read; twenty get swiped past, and then the real one gets swiped past too.
4. **Documents**, for the PDFs somebody might ask you for. One card per person.
5. **Sheets**, for everything else: bookings, phrases, budget, packing, contacts. No shell support
   needed, so this is where a pack grows for free.
6. **A theme**, last, and only if you want one.

## Writing a pack with an LLM

This is the intended way, and `skills/write-a-deskpress-pack/SKILL.md` is the file to hand it: it
carries the format, the rules and the failure modes in the order a model needs them.

A prompt that works:

> Read `skills/write-a-deskpress-pack/SKILL.md`, then read every file in `./notes/`. Build a pack at
> `./my-pack/`. Ask me about anything you cannot source from my notes instead of inventing it. Run
> `deskpress validate ./my-pack` and fix what it reports.

Three habits make the difference between a pack that reads like yours and one that reads like a
brochure:

- **Never invent a fact.** A price nobody confirmed is written `[to confirm]`, which renders as a
  visible hole. A model that guesses a hotel's phone number has made the app worse than a blank one.
- **Keep your own words.** The shell never parses prose, so there is no reason to smooth it out. The
  note that says "the market is closed on Sundays, do not promise the kids" is better content than
  any rewrite of it.
- **Validate before showing anybody.** `deskpress validate <pack>` costs a second and catches
  the whole class of mistakes that otherwise surface as a blank screen.

## Your language

Values are free: any language, any script, mixed inside one pack. Two things to know.

The **keys** are English. If your notes already use key names in your own language, do not rewrite
the file: declare a `keymap` in the manifest and the loader translates once, at load.

```yaml
keymap:
  root: {days: dias, places: lugares, people: viajeros}
  day: {date: fecha, title: titulo, blocks: bloques}
```

Root collections go under `root`, and every other key under the kind of thing it belongs to. The
full list is in [pack-format.md](pack-format.md).

The shell's own handful of labels (Today, Back, and a few more) follow `pack.language`, and `ui:` in
the manifest overrides any of them. If your language is not one the shell ships, `ui:` is how you
get it: four lines, and the app speaks your language.

## Mistakes worth naming

- **Blocks with a third element that is not a map.** The most common error by a distance. A block is
  time, text, and optionally a map: `["09:00", "text", {type: visit}]`.
- **Times as numbers.** `9:00` unquoted is not what you meant in YAML. Quote every time.
- **An alert for everything.** See above. Severity is a budget, not a label.
- **Options that are not really closed plans.** An option day is two plans you would be happy with.
  If one of them is "maybe we walk around", that is not an option, it is a free block.
- **Putting a decision in the pack.** The pack is a document. Which option you chose lives on the
  device, so the same pack works for everyone carrying it.
- **A place with no `during`.** It loads, and the moment then shows only your block's text. Fine for
  a coffee stop, a waste for the thing you traveled to see.

## Check it on the phone before you need it

Load the pack, then set the phone's clock forward to a moment you care about and look at what comes
up. It takes a minute and it is the only test that matters: at 08:40 on the day of the early train,
is the answer on the screen the thing you would have asked for?

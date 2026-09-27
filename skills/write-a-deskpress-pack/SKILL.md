---
name: write-a-deskpress-pack
description: Write or extend a deskpress pack, the YAML file that turns the deskpress shell into somebody's own offline app. Use it when the person wants an app for a trip, a treatment, a move, a season, a shift rota or any other stretch of days with facts attached, or when they hand over notes and ask for a pack.
---

# Writing a pack

A pack is a folder with one YAML file in it. The shell reads it and becomes that app. Your job is to
turn somebody's notes into that file, without inventing anything.

Read `docs/pack-format.md` at the repo root for the full key reference before you write. This page is the order of
work and the mistakes worth avoiding.

## The rule that decides everything

**At what moment does this matter?** Every fact goes where the moment that needs it can reach it. A
booking code belongs to the morning it is used, not to a list of codes. A phrase belongs to the meal.
A door code belongs to the arrival, at 23:00, when nobody has signal.

A fact with no moment was never going to be read. Leave it out.

## Order of work

1. **Read every note first.** All of it, before writing a line of YAML. The shape of the pack comes
   out of the notes, not out of the schema.
2. **Pick the template.** A trip is `extends: travel` under `pack:`, and the pack needs no
   `rules` or `screens`. Write `ui` labels only for a language other than English.
3. **List the days.** One entry per date, with a title that says what that day is. A day is the
   spine; everything else hangs off it.
4. **Write the blocks.** Time, text, in the order they happen. Keep the person's own words: the
   shell never parses prose, so there is nothing to gain by smoothing it out, and plenty to lose.
5. **Add places** for anywhere that needs something on arrival, and attach them with `place:`.
6. **Add coordinates** where being there changes the answer: `at: {lat, lon, radius_m}`. Only for
   places you have real coordinates for.
7. **Add alerts**, four or five, not twenty. See the budget rule below.
8. **Add documents** for the files somebody might be asked to show.
9. **Add climate** when the weather changes the plan: what each place is usually like that month,
   and a date only when you know better. Leave out what you do not know; a forecast can sync later.
10. **Put everything else under its own top level key.** Bookings, phrases, budget, packing, contacts.
   They become sheets with no shell support needed.
11. **Run the validator** and fix what it reports: `deskpress validate <pack>` (from a deskpress checkout, `cargo install --path crates/cli` puts it on the PATH). Errors block the
   load. Warnings are the honest backlog.

## Never invent a fact

This is the part that makes a pack worth carrying.

- A price, a phone number, an opening time or an address that is not in the notes goes in as
  `[to confirm]`, which renders as a visible hole. A guessed one is worse than a hole: it looks
  answered.
- Where a fact was checked and when, put `verified: YYYY-MM-DD` on the place. A card older than a
  month says so on screen.
- When something is genuinely undecided, do not pick for them. If there are two real plans, that is
  an option day (below). If there is one plan with a gap, that is a hole.
- **Ask.** A short list of questions at the end beats a pack with invented details in it.

## The language of the pack

Keys are canonical English; values are in whatever language the person wrote. If their notes already
use their own key names, **do not translate the file**: write a `keymap` in the manifest, grouped by
context, and the loader does it once at load. See `docs/pack-format.md`.

Enum values are structure, not prose: the thirteen `type` values and the four severities are English,
or mapped under `keymap.values`.

The shell's own labels follow `pack.language`, and `ui:` in the manifest overrides any of them.

## Alerts are a budget

Four real alerts get read. Twenty get swiped past, and then the real one gets swiped past too.

| severity | use it for | where it lands |
| --- | --- | --- |
| `critical` | expensive and imminent, a handful in a whole pack | above the answer |
| `high` | worth interrupting the moment | under the moment's cards |
| `medium` | worth a row in the day | the agenda |
| `low` | worth having written down | the alerts sheet |

## A day with options

Use it when a day is genuinely two plans, each one closed and each one acceptable. Not for "maybe we
also walk around": that is a block.

```yaml
- date: 2026-10-05
  title: Coast or the mountain
  fixed:  [["08:30", "Pick up the car."], ["20:00", "Dinner, all four."]]
  options:
    - id: coast
      name: The coast
      recommended: true
      why: "three driving days follow"
      blocks:
        - ["10:00", "Down the coast road. Stop where it looks worth stopping.", {type: driving}]
        - ["13:30", "Lunch on the beach side, the one with the shade.", {type: meal}]
    - id: mountain
      name: The mountain
      cost: "74 EUR"
      blocks:
        - ["09:45", "Rack railway up. Buy the return at the bottom.", {type: transfer}]
        - ["12:30", "Lunch at the top, the self service one.", {type: meal}]
  decision: {when: 2026-10-01, at: "21:00", decides: [2026-10-12]}
```

- Mark exactly one option `recommended`. Until somebody chooses, that is how the app behaves.
- `fixed` is what happens either way, and the only thing synced to the calendar up front.
- If another day's options depend on this answer, list that date under `decides` and put
  `requires: {date: ..., option: ...}` on the options of the dependent day.
- Never write the choice into the pack. It lives on the device: the pack is a document.

## Kid mode is filtering, not rewriting

If the pack has people with `adult: false`, a point is offered to them only when it carries
`for_kids`. Write that line where there is something real to offer, and leave it out where there is
not. A place with nothing for a kid is not a gap to fill with a made up game.

## Before you hand it over

- `deskpress validate <pack>` passes with no errors.
- Every `[to confirm]` is deliberate, and you listed them for the person.
- No fact in the pack is one you made up, including phone numbers, prices and hours.
- The day you can check is the one that matters: open the moment they would ask about, at the hour
  they would ask it, and see whether the answer is the thing they wanted.

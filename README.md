# deskpress

Your own app, printed from one file.

deskpress is an offline-first app shell. You write a YAML file that says what you need to know and
when you need it, load that file into the installed app, and the app becomes that app. No account,
no server, no store listing, no code. The shell is the press; what you print with it is yours.

It exists because the apps that would answer a handful of personal questions are never worth
building one at a time: a trip, a hospital stay, a move, a season of a sport, a treatment
schedule, a farm. The work in each of them is the same work, and none of it is the content.

## One question per screen

The idea the whole shell is built on: at any moment there is exactly one live question. What time
do we leave. Where did we park. Which dose is next. Who is on call. The app answers that one, in
the first screen, at 64px, before you finish taking the phone out of your pocket.

```
screen = view(moment, user)
moment = (day, time) + place
user   = who is holding the phone + theme + mode
```

Everything else follows from that. Six views cover every screen, because forty four screens drawn
on paper turned out to be six views with different data in them. There is no menu and no home: the
only way out of any screen is Today.

## How it works

1. **Write a pack.** One YAML file with your days, your places and your people, plus an optional
   theme file. Any language: the shell never reads your prose, it only places it. An LLM can write
   the whole thing from a folder of your own notes, and `skills/write-a-deskpress-pack` is the
   skill that tells it how.
2. **Validate it.** `node tools/validate.mjs <pack>` is the same validator that ships inside the
   app, so a pack that passes on your desk passes on the phone. It checks the skeleton, resolves
   every reference, and measures the contrast of your theme before you ever see it in the sun.
3. **Load it.** Open the installed app, point it at the file, and that is the app now. Replacing
   the file replaces the app. Nothing is compiled and nothing is uploaded.

## What the shell brings

- **Six views**: moment, sheet, agenda, suggestion, blocker, handoff. You do not choose one: the
  clock, the place and the pack choose it.
- **Twelve components** and a token based theme engine. A theme is data too: declare your colors
  once and no component branches. One theme per person is normal here, not an edge case.
- **A kid mode** that is an attribute, not a second app: bigger type, thicker borders, cards that
  become kid boxes, and content filtered to what was written for them.
- **Offline first, not offline capable.** The pack lives on the device. There is no request to make
  and no cache to warm. The only thing that ever reaches the network is opening a map, and when
  there is no signal that button shows the address as text instead.
- **Location, on the device.** Places can carry coordinates and a radius. The shell uses them to
  tell which place you are actually at, to mark a zone as safe for a kid session, and to remember
  where you left the car. Geofences are an OS API: nothing is reported anywhere.
- **Calendar sync, on demand.** A tool, not a background service: it writes your timed blocks into
  a named device calendar with stable ids, so syncing twice updates instead of duplicating.
- **A validator inside the app**, so loading a file that you or an LLM just wrote tells you what is
  wrong with it in plain words, instead of showing you a blank screen at the worst moment.

## Where to read next

| document | what it covers |
| --- | --- |
| [docs/architecture.md](docs/architecture.md) | the shell: reader, views, theme engine, tools, validator |
| [docs/pack-format.md](docs/pack-format.md) | every key the shell knows, and the rule for the keys it does not |
| [docs/authoring.md](docs/authoring.md) | how to write a pack, by hand or with an LLM |
| [schema/](schema/) | the machine readable contract, for an editor or an LLM. The validator carries the same rules in code |
| [tools/](tools/) | the YAML reader, the validator, and their tests. `node --test` from the repo root runs them |
| [examples/one-day/](examples/one-day/) | a whole pack, small enough to read in a minute, and it validates with no warnings |
| [skills/write-a-deskpress-pack/](skills/write-a-deskpress-pack/) | the skill an LLM loads before writing somebody's pack |

## Status

Early. The design system and the data contract are finished and measured against a real pack; the
Android shell is not written yet. The first pack in use is a private one, thirteen days of family
travel, and every decision in here came out of it.

## License

MIT. See [LICENSE](LICENSE).

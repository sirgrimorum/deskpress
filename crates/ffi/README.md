# deskpress-ffi

The engine as an app sees it, through [UniFFI](https://mozilla.github.io/uniffi-rs/). It only
converts: every rule lives in `deskpress-engine`, which never depends on UniFFI.

| export | does |
| --- | --- |
| `load(manifest, files)` | validates the pack; `LoadedPack`, or `Unreadable` / `Invalid` with the errors |
| `LoadedPack.warnings()` | what the validator warned about |
| `LoadedPack.id()` | the pack's id, which names where the host keeps its facts |
| `LoadedPack.theme()` | the theme file as written, `Null` when the pack has none |
| `LoadedPack.timezone()` | the pack's timezone, so the host can say what time it is there |
| `LoadedPack.zones()` | the other zones the days and blocks name; the host passes the local time of each in `World.zones` |
| `LoadedPack.screen(world)` | the view for that world: the tree to draw and its `watch` |
| `LoadedPack.calendar(world, scope, known)` | the calendar sync for a scope (the trip, a date, an event id) against the events the host wrote: a `Plan` to add, change and remove. An `Event`'s start and reminder are local to its `zone`, its end to `end_zone` |
| `LoadedPack.hosts()` | every host the pack fetches from, for the person to approve once |
| `LoadedPack.requests(world, module)` | the fetches a module syncs with now; an empty module asks for the automatic syncs due |
| `LoadedPack.received(world, request, status, body)` | the facts to store after a fetch |
| `LoadedPack.dispatch(world, action, arg)` | runs an action of the screen: the new view, a store patch and commands; `Refused` when the screen has no such action |
| `LoadedPack.alarms(world)` | every alarm still to come, by time: jet-lag steps and alerts that ask for one, and set-off notices |
| `LoadedPack.share(facts, stamps, files)` | the trip as text for another phone: the facts but the phone's own, when each was kept, and the pack's files when it says when it was updated |
| `LoadedPack.take(sent, facts, stamps)` | what this phone takes of a trip sent: each fact kept there later, and the files of a newer pack once they load; `Refused` when it is not a trip of this pack |
| `edit_theme(manifest, files, path, value)` | sets one value of the theme file in its text and checks the pack with it; the file and its new text, or the loader's refusal |
| `encode_facts(store)` / `decode_facts(text)` | the stored facts as text for a file on the device, and back; a damaged file holds none |
| `tree_version()` | the tree version this engine produces |

`make bindings` builds it for Android and writes the Kotlin bindings; `uniffi.toml` sets their
package. Values cross as the enum `Value` (`Null`, `Bool`, `Number`, `Text`, `Items`, `Fields`),
named so the Kotlin types `List` and `Map` stay unshadowed. Module actions returned by `dispatch`
cross as `Command { name: String, args: HashMap<String, Value> }`. The pack keeps its nav stack
between calls. The `uniffi-bindgen` binary needs `--features cli`, so plain builds skip it.

# deskpress-ffi

The engine as an app sees it, through [UniFFI](https://mozilla.github.io/uniffi-rs/). It only
converts: every rule lives in `deskpress-engine`, which never depends on UniFFI.

| export | does |
| --- | --- |
| `load(manifest, files)` | validates the pack; `LoadedPack`, or `Unreadable` / `Invalid` with the errors |
| `LoadedPack.warnings()` | what the validator warned about |
| `LoadedPack.timezone()` | the pack's timezone, so the host can say what time it is there |
| `LoadedPack.screen(world)` | the view for that world: the tree to draw and its `watch` |
| `LoadedPack.dispatch(world, action, arg)` | runs an action of the screen: the new view, a store patch and commands; `Refused` when the screen has no such action |
| `tree_version()` | the tree version this engine produces |

`make bindings` builds it for Android and writes the Kotlin bindings; `uniffi.toml` sets their
package. Values cross as the enum `Value` (`Null`, `Bool`, `Number`, `Text`, `Items`, `Fields`),
named so the Kotlin types `List` and `Map` stay unshadowed. The pack keeps its nav stack between
calls. The `uniffi-bindgen` binary needs `--features cli`, so plain builds skip it.

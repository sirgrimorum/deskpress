# deskpress-ffi

The engine as an app sees it, through [UniFFI](https://mozilla.github.io/uniffi-rs/). It only
converts: every rule lives in `deskpress-engine`, which never depends on UniFFI.

| export | does |
| --- | --- |
| `load(manifest, files)` | validates the pack; `LoadedPack`, or `Unreadable` / `Invalid` with the errors |
| `LoadedPack.warnings()` | what the validator warned about |
| `LoadedPack.screen()` | the screen tree to draw |
| `tree_version()` | the tree version this engine produces |

`make bindings` builds it for Android and writes the Kotlin bindings; `uniffi.toml` sets their
package. The `uniffi-bindgen` binary needs `--features cli`, so plain builds skip it.

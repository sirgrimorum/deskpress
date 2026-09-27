# 0011: the Android host, the ffi crate, and one Makefile

Status: accepted

## Context

Phase 2 is the spike: the engine loaded through UniFFI on Android, one screen drawn from a tree the
engine produced. It needs a crate that speaks UniFFI, an app, a way to build both, and a way to
check that the phone really shows the screen.

## Decision

- **`crates/ffi`** is the only crate that depends on UniFFI. It converts the engine's types to
  records and objects and holds no rule. It also ships the bindgen binary, behind the `cli`
  feature so a plain build never compiles it. The Kotlin package is `dev.deskpress.engine`.
- **The app** (`apps/android`) is Kotlin and Compose on AGP's built-in Kotlin. One thin
  `PackViewModel` loads the pack off the main thread and exposes a `StateFlow` of `Loading`,
  `Failed` or `Showing`; the screen draws whatever tree it holds. A tree version newer than the
  renderer knows is refused, not half drawn.
- **The spike screen** is `outline`: the pack's name, then one row per day, read through the
  keymap. Phase 3 replaces it with the screen the pack's rules pick.
- **Tests at three levels.** The engine at 100% in Rust. The app's JVM tests call the real engine,
  the desk build loaded through JNA, with no mocks. The device is checked by Maestro flows in
  `apps/android/flows`, run by `make e2e`: one flow per screen, added with the screen, so the flows
  are the regression suite.
- **One Makefile** holds every development task, named by use: `setup`, `check`, `ci`, `run`,
  `e2e` and the rest. It wraps shell commands and never the `deskpress` CLI. It replaces
  `scripts/check.sh`.
- **Latest stable, pinned where it lives.** The NDK, the SDK platform, the build tools and Maestro
  in the Makefile; Gradle in its wrapper with a checksum; the app's libraries in the version
  catalog; the build JDK (25) in `gradle-daemon-jvm.properties`, provisioned by Gradle itself.

## Consequences

- Generated files stay out of git: the bindings and the `.so` files are rebuilt by
  `make bindings`, so they can never drift from the engine.
- `make ci` needs the Android SDK; `make check` does not, so engine work stays fast.
- The app's Kotlin has no coverage gate yet. Its logic is two small functions, both tested; the
  rest is layout, which the Maestro flows check on a device.
- A device run needs an emulator with hardware acceleration or a phone over USB.

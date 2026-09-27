# deskpress for Android

The host: it reads the pack, hands it to the engine, and draws the tree the engine returns. It
holds no rule. See [decision 0011](../../docs/decisions/0011-android-host-and-make.md).

| file | what it is |
| --- | --- |
| `app/src/main/kotlin/.../Pack.kt` | `PackState`, `attempt` (engine failures become a state), the world, value text, the assets reader |
| `app/src/main/kotlin/.../PackViewModel.kt` | holds the pack and the store; calls the engine on a tap or when `watch.until` comes, with one timer |
| `app/src/main/kotlin/.../Screen.kt` | the renderer: one composable per node kind |
| `app/src/generated/` | the UniFFI bindings, written by `make bindings`, not in git |
| `app/src/main/jniLibs/` | the engine for each ABI, written by `make bindings`, not in git |
| `app/src/test/` | JVM tests against the real engine, the desk build loaded through JNA |
| `flows/` | Maestro flows: what a user sees on each screen. The device regression |

Everything runs from the repo root through `make`: `make android` tests and builds, `make run`
installs and opens the app, `make e2e` runs the flows. `make setup` installs what they need.
The flows freeze the clock: the launch extra `now`, a pack-local `YYYY-MM-DDTHH:MM`, replaces the
device time. The build JDK is fetched by Gradle; the wrapper only needs Java 17 or newer to start.

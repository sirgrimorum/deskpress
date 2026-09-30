# deskpress for Android

The host: it reads the pack, hands it to the engine, and draws the tree the engine returns. It
holds no rule. See [decision 0011](../../docs/decisions/0011-android-host-and-make.md).

| file | what it is |
| --- | --- |
| `app/src/main/kotlin/.../MainActivity.kt` | the shell: the bundled example (edits in an overlay) or the folder picked through the menu, the check and design screens, biometric/credential unlock, lock task, alarm sound/vibration, the dialer, the document viewer, the map app, the location permission and updates while started, and the approved hosts per pack (`files/hosts/<pack id>.txt`) |
| `app/src/main/kotlin/.../Location.kt` | the distance between two points, the regions a position is inside, smallest first, the pinned position, the pretend strip's words and the day's stops as a directions link |
| `app/src/main/kotlin/.../Settings.kt` | what the shell remembers whatever pack is open (`Shell`): the text size, the map app, the pretend clock and the pretend place |
| `app/src/main/kotlin/.../Assistant.kt` | what the phone's model is given: the host's orders apart from the pack's facts |
| `app/src/main/kotlin/.../Calendar.kt` | the calendar sync: the ledger of rows this device wrote per pack (`files/calendar/<pack id>.tsv`), applying a plan to only those rows, the calendar picker and the confirm dialog |
| `app/src/main/kotlin/.../Sync.kt` | the data sync's pieces: the HTTPS `GET` (15 s timeouts, 1 MB cap, no redirects), the secrets sealed with a Keystore key (`files/secrets/<pack id>/<host>/<name>`, so each goes only to the host it was given for), and the dialogs that approve the hosts and ask for a secret |
| `app/src/main/kotlin/.../Document.kt` | a pack PDF full screen, page by page through `PdfRenderer`, at maximum brightness |
| `app/src/main/kotlin/.../Design.kt` | the design system screen: the holder's tokens, a preview of each component, and the value editor, which offers to save a copy where the person picks when the folder can no longer be written |
| `app/src/main/kotlin/.../Pack.kt` | `PackState`, `attempt` (engine failures become a state), the world, value text, the assets and folder readers |
| `app/src/main/kotlin/.../PackViewModel.kt` | holds the pack and the store; calls the engine on a tap or when `watch.until` comes, with one timer, passing the local time of each zone the pack names; dispatches the host commands `device.unlock`, `phone.call`, `document.open`, `location.get`, `map.open`, `map.route`, `calendar.sync` and `<module>.sync`; fetches the automatic syncs that are due, after the activity's `allow` and `secret`; asks again when a new position changes the regions the device is inside; the clock set by hand adds `shift`; `Facts` keeps the store per pack id |
| `app/src/main/kotlin/.../Theme.kt` | the theme file read into tokens, the theme picked for the holder, the built-in neutral theme |
| `app/src/main/kotlin/.../Style.kt` | the fonts, a type step as a text style, hard shadows |
| `app/src/main/kotlin/.../Screen.kt` | the renderer: the Screen node as top and bottom bars, one composable per node kind, the menu |
| `app/src/main/res/font/`, `app/src/main/assets/licenses/` | Atkinson Hyperlegible Next and Jersey 10, with their OFL licences |
| `app/src/generated/` | the UniFFI bindings, written by `make bindings`, not in git |
| `app/src/main/jniLibs/` | the engine for each ABI, written by `make bindings`, not in git |
| `app/src/test/` | JVM tests against the real engine, the desk build loaded through JNA. Kover holds the code that does not draw to the line coverage measured when the gate was set (`app/build.gradle.kts`) |
| `flows/` | Maestro flows: what a user sees on each screen. The device regression |

Everything runs from the repo root through `make`: `make android` tests, checks the coverage floor and builds, `make run`
installs and opens the app, `make e2e` runs the flows. `make setup` installs what they need.
The flows freeze the clock: the launch extra `now`, a pack-local `YYYY-MM-DDTHH:MM`, replaces the
device time, and the app does not pin itself for a child, since the system's pin dialog outlives the
app. Under a frozen clock the position is not tracked either: the extra `at`, `lat,lon`, pins it. The build JDK is fetched by Gradle; the wrapper only needs Java 17 or newer to start.

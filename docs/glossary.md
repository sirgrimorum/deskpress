# Glossary

Two lists: the words deskpress made its own, then the technical terms the repo leans on. Each
entry is the short answer; the linked document has the rest.

## deskpress terms

| term | meaning |
| --- | --- |
| **action** | something that can happen on a screen, like `confirm`. A named list of effects |
| **alert** | a warning the pack wrote, shown by severity (`critical` to `low`) and by time. Never computed |
| **as of** | when synced data was fetched. Shown so a stale forecast reads as stale |
| **block** | one line of a day: a time, a text, and an optional map that makes it a moment |
| **canonical key** | the English key the shell reads, like `days` or `title`. Values can be in any language |
| **climate** | the module that answers "what is the weather here, today", from the pack, a sync, or both |
| **command** | what an action asks the host to do, like `map.open`. The engine names it; the host runs it |
| **component** | one of the closed set of things a screen can contain: `BigValue`, `Row`, `Button`... It grows only with a shell release |
| **content** | the pack's data, in `content.yaml`: days, places, people, alerts, documents, sheets |
| **day** | one entry of `days`: a date, a title, and its blocks. The spine of a travel pack |
| **definition** | the part of `pack.yaml` that says how the app behaves: modules, derive, rules, screens |
| **derive** | extra names a pack computes from expressions, in order, like `kid: not holder.adult` |
| **design system section** | the app's screen that shows every token and component of the theme and lets you edit them |
| **document** | a file that must open offline, like an insurance PDF, with its key fields as a card |
| **effect** | one step of an action: `set`, `store`, `open`, `back`, `home`, or a module action |
| **data mode** | where a module's data comes from: the pack (`from`), a `sync`, or both. Five modes, decision 0012 |
| **engine** | the Rust core. Loads and validates a pack, and turns it and the world into a screen tree. Pure: no clock, file or sensor |
| **error** | a problem that stops a pack from loading. Compare **warning** |
| **expression** | a small formula in a pack, like `decision.due and not decision.answered`. Parsed at load, never run as code |
| **host** | the app around the engine (Android today). Owns the device: files, clock, location, storage, tools |
| **holder** | the person holding the phone right now. Screens can change with who it is |
| **keymap** | a map in `pack.yaml` from canonical keys to the pack's own key names, grouped by context |
| **kid mode** | the screens shown when the holder is not an adult |
| **ledger** | what the host wrote into the calendar for one pack: the calendar picked, and a row and fingerprint per event id. A sync touches only those rows |
| **main machine** | the pack's `rules`: it answers "which screen, right now" |
| **manifest** | `pack.yaml`: the pack's name, language, timezone, where its content is, and its definition |
| **module** | shared, tested logic a pack opts into: `timeline`, `choices`, `people`, `places`, `alerts`, `documents`, `climate` |
| **moment** | the travel screen for the block happening now. Its cards depend on the block's type |
| **moment type** | one of the thirteen block types (`visit`, `train`, `meal`...). Each picks the cards of its moment |
| **nav stack** | the screens a user opened, first the one the rules picked. `open` pushes, `back` pops, `home` leaves the first |
| **node** | one component in a screen tree, with its props and the events it answers |
| **outline** | the screen of a pack with no `rules`: its name, then one row per day |
| **params** | the values `open` passed to the screen it pushed. Empty on a screen the rules picked |
| **pack** | a folder that describes an app: `pack.yaml`, `content.yaml`, optional `theme.yaml` and files. The shell loads it and becomes that app |
| **path reference** | a value like `bookings.azulejo` that points somewhere else in the content; a card shows what it points to |
| **renderer** | the part of the host that draws a screen tree. Knows components and tokens, nothing about packs |
| **rule** | one line of the main machine: a `when` and a `screen`. The first true one wins |
| **screen** | one view of the app, defined in the pack with its `state`, `actions` and `layout` |
| **screen machine** | a screen's own state and actions: it answers "what can happen here" |
| **screen tree** | what the engine hands the renderer: plain, versioned data listing the nodes to draw |
| **sheet** | any other tree in the content (bookings, phrases, contacts), shown as rows and cards with no shell support needed |
| **secret** | a key a sync needs, named in the pack with the query parameter it goes in, and typed on the device. Kept sealed with a keystore key, never in the pack |
| **shell** | the engine plus a host: everything that is the same for every pack |
| **stored fact** | a value the app saved on the device, like a decision. It is an input, so it can change the screen |
| **template** | a definition bundled with the engine, like `travel`, that a pack names in `pack.extends` and only overrides where it differs. See `docs/templates.md` |
| **theme** | the design system as data, in `theme.yaml`: color, type, spacing and radius tokens |
| **token** | one named value of the theme. Components read tokens and never a theme's name |
| **sync** | fetching a module's data from a source the pack names, on a button or automatically every so often while the app is open. Only `climate` so far |
| **tool** | a device capability the engine can ask the host for: map, calendar, geofence, fingerprint |
| **tree version** | the number of the screen tree's shape. A renderer refuses a newer one instead of half drawing it |
| **unknown key rule** | a key the shell does not know becomes a card labelled with that key, or an alert by its prefix or date. `Auto` applies it. Enriching a pack never breaks it |
| **validate** | load a pack and list its errors and warnings. `deskpress validate <pack>` on the desk, `load` in the app |
| **warning** | something worth fixing that does not stop the pack from loading, like a place with no coordinates |
| **watch** | what comes with each tree to say what would change it: the next instant (`until`) and the geofences that matter. The host calls the engine again only then |
| **world** | what the host pushes in on each call, as opposed to a module's data: the local time in the pack's timezone and in each zone the days name, the places whose region the device is inside and whether it is located, the holder and the stored facts |
| **write back** | a theme edit in the app is saved to the loaded theme file, keeping its comments and order |

## Technical terms

| term | meaning |
| --- | --- |
| **ABI** | the processor family a native library is built for. The app ships `arm64-v8a` (phones) and `x86_64` (emulators) |
| **adb** | Android's command line link to a phone or emulator: install, open, read logs |
| **AGP** | the Android Gradle Plugin: what teaches Gradle to build an Android app |
| **bindings** | the Kotlin code that lets the app call the Rust engine as if it were Kotlin. Generated by UniFFI, never written by hand |
| **cargo-ndk** | a cargo helper that builds Rust for Android with the NDK |
| **Compose** | Android's UI toolkit: the screen is a function of the state, redrawn when the state changes |
| **coverage (regions)** | how much of the code the tests run. Regions count each side of every condition; the engine is gated at 100% |
| **crate** | a Rust package. The repo has three: `engine`, `cli`, `ffi` |
| **e2e flow** | a Maestro script that drives the real app on a device and checks what the user sees. The regression suite |
| **FFI** | foreign function interface: how code in one language calls code in another |
| **functional core, imperative shell** | all logic in pure functions (the engine), all I/O at the edge (the host) |
| **JNA** | a Java library that loads a native library and calls it. The bindings use it, on the phone and in the JVM tests |
| **NDK** | Android's kit for building native (non Java) code. Needed to compile the engine for phones |
| **StateFlow** | a Kotlin value that can be watched: the ViewModel sets it, Compose redraws when it changes |
| **UniFFI** | a Mozilla tool that reads the Rust library's exported types and functions and writes bindings for Kotlin, Swift and others |
| **ViewModel** | an Android class that keeps state alive while the screen rotates or is rebuilt. Here it only carries state |

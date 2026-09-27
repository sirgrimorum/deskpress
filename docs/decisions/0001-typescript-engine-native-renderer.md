# 0001: One TypeScript engine, one native renderer per platform

Status: superseded by [0010](0010-rust-core-uniffi.md), 2026-09-26. Accepted earlier the same day.

## Context

deskpress has two very different halves. The **engine** holds nearly all the complexity: parsing,
validation, expressions, modules, the rule table, the screen machines. It must run in four places:
on a desk (CLI), inside the app, inside an LLM plugin, and in a browser preview. The **renderer**
is small: a closed set of themed components that draws a tree the engine hands it.

Two facts from 2026 moved the choice. Shopify moved its apps from React Native back to Swift and
Kotlin because coding agents made building each platform cheap, while native still gives first
party APIs and fewer dependency layers ([decision](https://shopify.engineering/back-to-native),
[Shop app](https://shopify.engineering/shop-app-migration)). And their core principle was business
logic decoupled from the UI, headless, behind a CLI, so an agent can iterate in milliseconds.

Android specifics favour native here too: geofences through `GeofencingClient` fire even when the
app is not running, persisted document permissions let the app write back to the exact file the
person picked, and `BiometricPrompt`, brightness and `CalendarContract` are used directly.

## Decision

- The engine is written **once, in TypeScript**, with no runtime dependencies. It is pure: data in,
  data out. It never touches a clock, a file or a sensor; the host passes those in.
- Each platform gets a **thin native renderer and host**. Android first: Kotlin and Jetpack
  Compose. The host runs the engine bundle in an embedded JS runtime and talks to it in JSON:
  `load`, `screen`, `dispatch`.
- The runtime candidate is [quickjs-kt](https://github.com/dokar3/quickjs-kt), because it also runs
  on the JVM, so the bridge is tested without an emulator. The fallback is
  [androidx.javascriptengine](https://developer.android.com/jetpack/androidx/releases/javascriptengine).
- Later platforms (iOS with SwiftUI and JavaScriptCore, a React web renderer for previews) are
  ported from the Android renderer, and kept equal by shared fixtures.

## Consequences

- One validator, one set of rules: a pack that passes on the desk passes on the phone.
- The renderer contract (the screen tree) is the seam. It is versioned and fixture tested.
- A JS runtime lives inside a native app. **Gate:** a one day spike must show the engine bundle
  loading in QuickJS on a device and one screen rendered by Compose, with the bridge under JVM
  tests. If it fails, the fallback is Expo, recorded as a new decision.
- Contributors to the app need Android Studio as well as Node.

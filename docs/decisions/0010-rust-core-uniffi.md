# 0010: A Rust core, loaded natively through UniFFI; Cargo for the repo

Status: accepted, 2026-09-26. Supersedes [0001](0001-typescript-engine-native-renderer.md) and
[0007](0007-monorepo-and-tooling.md). Amends how [0008](0008-testing.md) measures coverage.

## Context

0001 put a TypeScript engine inside an embedded JS runtime on the phone. It works, but it puts a
second runtime and a string bridge inside a native app: harder to debug across the seam, one more
thing to ship and to trust. The engine should load the way any native library does, on Android now
and on iOS later, and still run on a desk and in an LLM plugin.

Options weighed: Kotlin Multiplatform (the most native on Android, weaker on the web and the desk),
Go with gomobile (little maintenance, only basic types cross, a GC runtime in the app), and Rust
with [UniFFI](https://github.com/mozilla/uniffi-rs) (Mozilla's binding generator, used by the
Firefox apps and the Matrix and Bitwarden SDKs). Rust with UniFFI is native on every target and
compiles to a small Wasm module for the web.

## Decision

- **The engine is a Rust crate**, `crates/engine`: pure, std only, no I/O. Time, location, the
  holder and stored facts are arguments. The calls stay `load`, `screen`, `dispatch`.
- **The bridge is UniFFI**, in its own crate, `crates/ffi`, so the core never depends on it. It
  generates Kotlin (and later Swift) types for the calls, the world and the screen tree: the host
  gets typed values, not JSON to parse.
- **Android** builds `crates/ffi` with `cargo-ndk` for `arm64-v8a` (phones) and `x86_64`
  (emulator), and loads it as a normal `.so`. The generated Kotlin uses JNA, so the same bindings
  run in JVM tests against a desk build of the library, without an emulator.
- **The desk and the LLM plugin** use `crates/cli`, a native `deskpress` binary. It prints screen
  trees as JSON, which is what fixtures and LLMs read.
- **Later**: Swift bindings and an xcframework for iOS; a `wasm32` build for the web preview and
  an `npx` friendly plugin.

Tooling:

| concern | choice |
| --- | --- |
| workspace | Cargo, edition 2024, stable toolchain pinned in `rust-toolchain.toml` |
| format | rustfmt, width 100 |
| lint | clippy, every warning an error; `unsafe_code` forbidden in our crates |
| tests | `cargo test`: unit tests next to the code, binary tests in `tests/` |
| coverage | `cargo llvm-cov`, gated at 100% of lines, functions and regions |
| one command | `make check` (decision 0011) |
| Android | Gradle wrapper, Kotlin, Compose, `cargo-ndk` |

## Consequences

- No runtime inside the app and no string bridge; the screen tree is a typed value on both sides.
- Branch coverage needs a nightly compiler, so the gate uses **regions** on stable, which counts
  each side of every condition and is stricter than lines. Switch to branches when stable has it.
- `main` returns an `ExitCode` instead of calling `process::exit`, or the binary's coverage is lost.
- Contributors need Rust and, for the app, Android Studio. Node is needed only for `tools/`, until
  phase 1 ports them into the engine and removes them.
- **Gate:** phase 2 must show the engine loaded through UniFFI on a device, one Compose screen
  drawn from its tree, and the bindings under JVM tests. If it fails, the fallback is Kotlin
  Multiplatform, recorded as a new decision.

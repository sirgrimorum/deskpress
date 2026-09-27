# 0008: Tests start from what must not happen; the engine is gated at 100%

Status: accepted, 2026-09-26. Amended by [0010](0010-rust-core-uniffi.md): coverage is measured with `cargo llvm-cov`, the crates are `crates/engine` and `crates/cli`, and the JVM tests load the UniFFI bindings.

## Context

The failure this project exists to prevent is the wrong screen, or a blank one, at the worst moment.
Most of the risk is in what the engine must refuse or must not do: accept a malformed pack, crash on
a missing key, run an expression it should have rejected, lose a stored choice, show a sheet over a
decision that is due.

## Decision

- **Negative cases first.** For every unit, the first tests written are the ones that must fail:
  refused input, unresolved references, bad expressions, illegal actions, contrast below 4.5:1,
  prototype keys, oversized input. Happy paths come after.
- **Coverage gate.** `packages/engine` and `packages/cli` fail the build below 100% of lines,
  branches and functions, measured by `node --test --experimental-test-coverage`. A line that
  cannot be reached by a test is deleted, not excluded.
- **Fixtures as the contract.** Engine behaviour is pinned by fixture tables: a pack, a set of
  inputs, the expected screen tree. The same fixtures run against the engine in Node and inside the
  Android host on the JVM, which is what keeps the platforms equal.
- **Android.** JVM unit tests for the host and the bridge; Compose tests for each component;
  screenshot tests for the theme; one end to end flow on an emulator. Coverage is measured with
  Kover, and the threshold is set once the spike shows what is reachable on the JVM.
- **Private data never enters a test.** Fixtures are invented; `content/` is never read by a test.

## Consequences

- Every bug fix starts with the test that reproduces it.
- The gate makes dead code visible early, which is most of the tech debt this repo could grow.

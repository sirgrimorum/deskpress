# Contributing to deskpress

Read this before contributing. The repo follows strict design principles and quality gates.

## Architecture Principles

- **Functional core, imperative shell.** All domain logic lives in the engine (`crates/engine`), pure, standard library only. All I/O, device sensors, and network interaction live in the host (`apps/android`).
- **Unidirectional data flow.** The engine receives input data and outputs a complete screen tree; the renderer never decides or mutates anything.
- **Data driven UI.** The renderer maps components and tokens; it never learns domain pack keys.
- **Simplest thing that works.** No speculative abstraction, no unused layers.

## Development Setup

The repository uses GNU Make for all development workflows:

```sh
make setup        # Install Rust targets, cargo tools, Android SDK parts, and Maestro
make check        # The gate: format, clippy (-D warnings), 100% coverage
make android      # Run Android JVM tests, verify Kover coverage floor, and build APK
make e2e          # Run Maestro device regression flows
```

## Quality Gates

Every change must pass the gates:

1. **Rust Engine (100% Coverage):**
   `make check` requires 100% line, function, and region coverage across workspace crates. Negative cases must be tested first.
2. **Android Host:**
   `make android` runs unit tests against the real engine and verifies the Kover line coverage floor.
3. **End-to-End Regression:**
   A change to screen rendering or screen actions must update or add a Maestro flow under `apps/android/flows`.

## Writing Style & Conventions

- **English only.** Code, documentation, comments, and commit messages are written in English.
- **No em dashes.** Never use em dashes in prose or comments. Colons, semicolons, and commas do the job.
- **Mermaid diagrams only.** Architecture flows and state machines must use fenced `mermaid` blocks.
- **Commit format.** One-line commit messages in imperative mood: `<type>(<scope>): <subject under 72 chars>`.
- **Single branch workflow.** Development happens on `main`. Keep commits atomic and self-contained.

## Description

Brief summary of what this change does and why.

## New Features

List any new capabilities, components, expressions, or commands introduced (or "None").

## Breaking Changes

List any breaking schema, API, or behavioral changes (or "None").

## New Dependencies

List any new crate, library, or tool dependencies with rationale (or "None"). Note that `crates/engine` must remain standard library only.

## How to Test

Instructions and exact commands to verify this change:

```sh
make check
make android
```

## Checklist

- [ ] `make check` passes: format, clippy (-D warnings), 100% Rust test coverage.
- [ ] `make android` passes: JVM unit tests, Kover coverage floor, debug APK build.
- [ ] End-to-end regression: Maestro flows updated or added under `apps/android/flows` if UI or screen actions changed.
- [ ] No em dashes in prose or comments: use colons, semicolons, or commas instead.
- [ ] Diagrams use Mermaid syntax only.
- [ ] Documentation and decision records updated in the same change.
- [ ] Commit message follows format: `<type>(<scope>): <subject under 72 chars>`.

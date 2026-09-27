# Decisions

One file per decision that shapes the repo. Each is short: the context, what was decided, and what
it costs. A decision is changed by writing a new file that supersedes the old one, never by editing
history.

| id | decision | status |
| --- | --- | --- |
| [0001](0001-typescript-engine-native-renderer.md) | one TypeScript engine, one native renderer per platform | superseded by 0010 |
| [0002](0002-main-machine-and-screen-machines.md) | a guarded rule table picks the screen; each screen is its own machine | accepted |
| [0003](0003-expression-language.md) | a small expression grammar of our own | accepted |
| [0004](0004-modules-and-derive.md) | domain logic lives in built-in modules plus `derive` | accepted |
| [0005](0005-screens-compose-components.md) | a screen is a tree of the shell's components | accepted |
| [0006](0006-theme-edits-write-back.md) | theme edits in the app write back to the loaded file | accepted |
| [0007](0007-monorepo-and-tooling.md) | pnpm workspaces, Node's own TypeScript and test runner | superseded by 0010 |
| [0008](0008-testing.md) | tests start from what must not happen; the engine is gated at 100% | accepted, amended by 0010 |
| [0009](0009-documentation.md) | who each document is for, and what stays private | accepted |
| [0010](0010-rust-core-uniffi.md) | a Rust core loaded natively through UniFFI; Cargo for the repo | accepted |
| [0011](0011-android-host-and-make.md) | the Android host, the ffi crate, one Makefile, Maestro flows | accepted |

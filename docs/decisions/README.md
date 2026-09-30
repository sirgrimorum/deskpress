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
| [0012](0012-data-sources-and-sync.md) | a module's data comes from the pack, a sync, or both; the climate module | accepted, amended by 0022 |
| [0013](0013-call-only-on-change.md) | the engine is called only when its answer can change; each tree says when | accepted |
| [0014](0014-travel-template.md) | a pack extends a bundled template; the travel template is the first | accepted |
| [0015](0015-android-renderer.md) | the renderer draws from theme tokens; a pack comes from a picked folder; facts stay on the device | accepted |
| [0016](0016-theme-editor.md) | the design system screen edits one theme value at a time, through the engine | accepted, amended by 0023 |
| [0017](0017-kid-mode-and-handoff.md) | kid mode, handoff, and host device commands | accepted, amended by 0023 |
| [0018](0018-documents.md) | documents: numbers before the file, the file full screen offline, adults only | accepted |
| [0019](0019-group.md) | `Group`, the one component that holds others; tree version 4 | accepted |
| [0020](0020-location.md) | location: geofences only while open, `away`, the car's spot and the map | accepted |
| [0021](0021-calendar.md) | calendar sync: a picked account calendar, a ledger of the app's own events, a plan to confirm | accepted, amended by 0023 |
| [0022](0022-data-sync.md) | data sync: the engine builds requests and reads replies, the host fetches; climate first | accepted |
| [0023](0023-drift-review.md) | the drift review of phase 7: what was fixed, what is kept on purpose, the spec's extras as phases | accepted |
| [0024](0024-public-repo-ci-and-security.md) | the public repo: remote, CI for gate and Android, security audit, community files | accepted |
| [0025](0025-the-day-in-hand.md) | the day in hand: edits as facts keyed by `event`, checkable sheets, the set-off notice, two shell settings | accepted, amended by 0030 |
| [0026](0026-maps.md) | maps with nothing fetched: the day's chart, a place's plan, `road`, and the `Map` component | accepted |
| [0027](0027-the-assistant.md) | the assistant in three tiers: the pack's menu of questions, the phone's own model, and shortcuts outside the app | accepted, amended by 0030 |
| [0028](0028-performance-at-scale.md) | performance at scale: one calendar batch, one query per directory, a key per node; the stored facts measured and left where they are | accepted |
| [0029](0029-the-authoring-plugin.md) | the authoring plugin: the repo as plugin and marketplace, the binary from git, `reference` and `preview` in the CLI, no MCP server yet | accepted |
| [0030](0030-trying-a-moment-and-the-day-as-a-route.md) | a pretend clock and place in Settings, answers in a `Dialog`, the day's stops as a route, only the installed map apps | accepted |

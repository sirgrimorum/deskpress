# 0007: pnpm workspaces, Node's own TypeScript and test runner

Status: superseded by [0010](0010-rust-core-uniffi.md), 2026-09-26. Accepted earlier the same day.

## Context

The repo becomes a monorepo: an engine, a CLI, an Android app, later more renderers and an LLM
plugin. The existing tools ran with plain `node` and nothing to install, and that property is worth
keeping as far as it goes.

## Decision

| concern | choice | why |
| --- | --- | --- |
| workspaces | pnpm | strict, fast, one lockfile |
| language | TypeScript, `erasableSyntaxOnly` | Node 24 runs it directly by stripping types: no build step for tests or the CLI |
| type check | `tsc --noEmit`, strict | the only compile step on the desk |
| tests | `node:test`, coverage built in | no test framework to install or upgrade |
| lint and format | Biome | one binary for both, fast |
| engine bundle | esbuild, to one file | what the Android host loads into QuickJS |
| Android | Gradle wrapper, Kotlin, Compose | the platform's own tools |

Layout:

| path | what |
| --- | --- |
| `packages/engine` | yaml, expressions, loader, validator, modules, machines. No runtime deps |
| `packages/cli` | `deskpress validate`, `screen`, `act`: the engine on a desk |
| `apps/android` | the host and the Compose renderer |
| `templates/` | public pack definitions to start from, like `travel` |
| `examples/` | public example packs |
| `skills/` | what an LLM loads to write a pack; becomes a plugin later |

## Consequences

- Erasable syntax only: no `enum`, no `namespace`, no parameter properties. Unions of string
  literals do the job.
- Relative imports carry their `.ts` extension, because Node resolves real files.
- Runtime dependencies of the engine stay at zero; adding one is a decision.
- `tools/*.mjs` are ported into `packages/engine` with their tests, then removed.

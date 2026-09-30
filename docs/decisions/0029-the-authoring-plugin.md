# 0029: the authoring plugin

Status: accepted, 2026-09-30.

## Context

Phase 13: the skill plus the CLI, packaged for LLM hosts, done when an LLM writes, validates and
previews a pack end to end.

Two of the three already exist. `skills/write-a-deskpress-pack/SKILL.md` is the order of work, and
`deskpress validate` is the check. What is missing is everything that assumes a checkout:

- The skill says "read `docs/pack-format.md` at the repo root". An installed plugin has no repo root.
- The binary comes from `cargo install --path crates/cli`, which needs the checkout too.
- There is no preview a person can read. `screen` prints the tree as JSON: right for a renderer, and
  for a model checking a value, but not something to show the person whose trip it is.

## Decision

**A Claude Code plugin made of what the repo already has, a CLI that carries its own reference, and
a text preview. No MCP server yet.**

### The repo is the plugin and its marketplace

Two files at the root, `.claude-plugin/plugin.json` and `.claude-plugin/marketplace.json`, the
second listing the first with `source: "./"`. `skills/` is already where a plugin's skills go, so
nothing moves. The install is two commands:

```
/plugin marketplace add sirgrimorum/deskpress
/plugin install deskpress@deskpress
```

`plugin.json` carries no version. A fixed one stops updates until it is bumped, and the workspace
stays at 0.0.0; with none, a git marketplace versions the plugin by its commit. The binary is
built from the default branch when it is installed, so it can be ahead of the plugin.

### The binary is installed from the repo

Releases stay out of CI (0024), so there is no binary to download. The skill checks
`deskpress version` first and, when it is missing, installs from git:

```
cargo install --locked --git https://github.com/sirgrimorum/deskpress deskpress-cli
```

That needs a Rust toolchain. It is the honest price until releases exist, and it goes away then
without the skill changing shape. The plugin ships no `bin/`: a built binary in git goes stale
against the source, and claude.ai refuses plugins that carry one.

### The reference travels with the binary

`deskpress reference` prints `docs/pack-format.md` and `docs/templates.md`, compiled in with
`include_str!`. One source, no copy to drift, and the reference an LLM reads is the one the binary
enforces. The skill's first step becomes "run `deskpress reference`".

### A preview a person can read

`deskpress preview <pack> --at <time> [world] [action]...` takes the arguments of `screen`, and
actions as `act` does to reach another screen, and prints the screen as text: the title, then one
line per node with what that node shows, indented by group, and the foot. It is the tree already
built, drawn with no styling, so it stays in `crates/cli` and the engine does not change.

The skill's last step already says "open the moment they would ask about, at the hour they would
ask it". With `preview`, the LLM can do that and show the person the answer.

### No MCP server, for now

An MCP server would reach hosts with no shell, Claude Desktop the likeliest. It costs a JSON-RPC
loop and a JSON reader in a std-only workspace, plus tracking a protocol that is still changing.
The hosts that have a shell, which is where packs get written today, need none of it. It is the
next step when a host with no shell is actually wanted, and `validate`, `screen`, `preview` and
`reference` are already the four tools it would expose.

## Consequences

- `crates/cli` gains two commands, tested like the others, at 100%.
- The skill stops pointing at the repo: it runs `deskpress reference`, installs the binary when it
  is missing, and previews before handing over.
- `docs/authoring.md` and the README gain the two install lines.
- `validate` warns about a place that can never show: no block names it and it has no `at`. The
  first real pack tagged no blocks at all, and nothing said so.
- Done when, checked by hand and written in the pull request: in a fresh Claude Code session with
  only the plugin installed and no checkout, a model turns a short set of invented notes into a
  pack that validates with no errors, and previews one moment of it.

# deskpress-cli

The `deskpress` binary: the engine on a desk, for people and for LLMs writing packs.

| command | does |
| --- | --- |
| `version` | prints the screen tree version |
| `validate <pack>` | checks a pack folder, or its `pack.yaml`, and lists every problem |
| `screen <pack> --at <time> [world]` | the screen for a moment and its watch, as one line of JSON |
| `act <pack> --at <time> [world] <action>...` | runs actions in turn; prints the screen they leave, what they stored and the commands they asked for |
| `preview <pack> --at <time> [world] [action]...` | the screen for a moment, after any actions, as text a person reads: one line per component |
| `reference` | prints `docs/pack-format.md` and `docs/templates.md`, compiled in, so the reference is the one this binary enforces |

`<time>` is local to the pack, `YYYY-MM-DDTHH:MM`. `[world]` is any of `--holder <person id>`,
`--inside <place id>` and `--store <key>=<value>`. An action is a name, or `name=<value>` to send
it a value; a value is YAML. For example:

```sh
deskpress screen examples/one-day --at 2026-04-11T11:30
deskpress act examples/one-day --at 2026-04-11T11:30 points back
```

`run(args, out, err)` returns the exit code and takes its streams, so every path is a unit test.
Run it with `cargo run -q -p deskpress-cli -- <command>`, or install it with
`cargo install --path crates/cli`, or with no checkout:
`cargo install --locked --git https://github.com/sirgrimorum/deskpress deskpress-cli`.

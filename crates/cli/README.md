# deskpress-cli

The `deskpress` binary: the engine on a desk, for people and for LLMs writing packs.

| command | does |
| --- | --- |
| `version` | prints the screen tree version |
| `validate <pack>` | checks a pack folder, or its `pack.yaml`, and lists every problem |
| `screen <pack> --at <time>` | phase 3: the screen tree for a moment, as JSON |
| `act <pack> <action>` | phase 3: runs an action and prints the result |

`run(args, out, err)` returns the exit code and takes its streams, so every path is a unit test.
Run it with `cargo run -q -p deskpress-cli -- <command>`, or install it with
`cargo install --path crates/cli`.

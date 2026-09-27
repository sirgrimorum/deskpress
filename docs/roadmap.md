# Roadmap

What is done and what comes next. Each phase ends usable and tested; nothing is started on top of a
phase that is not.

| phase | what | done when |
| --- | --- | --- |
| 0 ✓ | decisions, docs, Cargo workspace skeleton | `make check` runs format, clippy and tests at 100% coverage on an empty engine |
| 1 ✓ | engine core: YAML reader, expressions, loader with keymap, validator | the ported validator passes the old tests; 100% coverage |
| 2 | Android spike: the engine through UniFFI, one Compose screen | a device shows a screen tree produced by the engine (`make e2e`); bindings tested on the JVM |
| 3 | modules (including `climate` from the pack), rule table, screen machines, screen tree; CLI `screen` and `act` | the example pack answers "what now" for any input on the desk |
| 4 | `templates/travel`: the six views as screens, the thirteen moment types | a travel pack needs only content and a theme |
| 5 | Android renderer: every component, theme engine, pack picker, validator screen | a pack loads from a file and runs offline |
| 6 | design system section: tokens and components live, edits written back | a theme edit survives a restart and passes contrast |
| 7 | tools: location and geofences, calendar sync, data sync by button or schedule (decision 0012), kid mode and handoff, documents | the travel brief is complete |
| 8 | the authoring plugin: the skill plus the CLI, packaged for LLM hosts | an LLM writes, validates and previews a pack end to end |
| 9 | web renderer for previews; iOS renderer; store builds | the same fixtures pass on every platform |

# Roadmap

What is done and what comes next. Each phase ends usable and tested; nothing is started on top of a
phase that is not.

| phase | what | done when |
| --- | --- | --- |
| 0 ✓ | decisions, docs, Cargo workspace skeleton | `make check` runs format, clippy and tests at 100% coverage on an empty engine |
| 1 ✓ | engine core: YAML reader, expressions, loader with keymap, validator | the ported validator passes the old tests; 100% coverage |
| 2 ✓ | Android spike: the engine through UniFFI, one Compose screen | a device shows a screen tree produced by the engine (`make e2e`); bindings tested on the JVM |
| 3 ✓ | modules (including `climate` from the pack), rule table, screen machines, screen tree; CLI `screen` and `act` | the example pack answers "what now" for any input on the desk, with the `watch` that says when it changes (decision 0013) |
| 4 ✓ | `templates/travel`: the six views as screens, the thirteen moment types (decision 0014) | a travel pack needs only content and a theme |
| 5 ✓ | Android renderer: every component, theme engine, pack picker, validator screen, stored facts kept on the device (decision 0015) | a pack loads from a file and runs offline |
| 6 ✓ | design system section: tokens and components live, edits written back (decision 0016) | a theme edit survives a restart and passes contrast |
| 7 ✓ | tools, in parts: 1 ✓ kid mode and handoff; 2 ✓ documents, grouped per person (decision 0019); 3 ✓ location: geofences while open, the car's spot, the map (decision 0020); 4 ✓ calendar sync: into a calendar the person picks, only the app's own events (decision 0021); 5 ✓ data sync: the climate module by button or while open, hosts approved, secrets sealed on the device (decision 0022); 6 ✓ drift review (decision 0023): every place the work left the brief or the design (the "Deviations from the design" of each decision, and anything found by going through the brief again) is listed, and each one is either fixed or kept on purpose in a decision | the travel brief is complete, and no drift from it is left undecided |
| 8 | the day in hand, the spec's small extras: checkable packing lists, a font size setting, a navigator choice, a "leave in five minutes" notice, moving or removing blocks at night | each one works offline on the example pack, with its flow |
| 9 | maps: offline and terminal maps, road content between places | a day's places and its terminal show with no network |
| 10 | the shared trip: a log with photos, synced between the phones through a bucket the family owns | a photo logged on one phone shows on the other |
| 11 | the assistant: questions about the trip answered from the pack, with menus and languages per person where a pack needs them | a person asks in their language and gets the pack's answer |
| 12 | performance at scale, found in the phase 7 review: calendar writes in one batch, a picked folder listed with one query per directory, the stored facts kept on the Rust side instead of crossing to Kotlin on every call, keys on the screen's list items | a trip of several hundred events syncs in about a second, and a large pack opens and draws as fast as the example |
| 13 | the authoring plugin: the skill plus the CLI, packaged for LLM hosts | an LLM writes, validates and previews a pack end to end |
| 14 ✓ | the public repo: remote, CI running gate and Android host tests, security audit, Dependabot, contribution files (decision 0024); releases kept out of CI for now | a push to the remote is checked |
| 15 | web renderer for previews; iOS renderer; store builds | the same fixtures pass on every platform |

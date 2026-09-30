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
| 8 ✓ | the day in hand, the spec's small extras (decision 0025): checkable packing lists, a text size setting, a navigator choice, a set-off notice, and moving, resizing, swapping, removing or adding blocks, every edit a stored fact keyed by the block's `event` | each one works offline on the example pack, with its flow |
| 9 ✓ | maps with nothing fetched (decision 0026): the day's places on plain paper, a place's own plan of pins over a picture the pack ships, and `road`, what a moving block passes | a day's places and its terminal show with no network |
| 10 | the shared trip: a log with photos, synced between the phones through a bucket the family owns | a photo logged on one phone shows on the other |
| 11 ✓ | the assistant in three tiers (decision 0027): the pack's menu of questions answered from its own data, the phone's own model where there is one, and the questions a launcher offers outside the app | a person asks in their language and gets the pack's answer |
| 12 ✓ | performance at scale, found in the phase 7 review (decision 0028): the calendar written in one batch, a picked folder listed with one query per directory, a key on the screen's list items, and the stored facts left where they are on the measurement | a trip of several hundred events syncs in about a second, and a large pack opens and draws as fast as the example |
| 13 ✓ | the authoring plugin: the skill plus the CLI, packaged for LLM hosts (decision 0029) | an LLM writes, validates and previews a pack end to end |
| 14 ✓ | the public repo: remote, CI running gate and Android host tests, security audit, Dependabot, contribution files (decision 0024); releases kept out of CI for now | a push to the remote is checked |
| 15 | web renderer for previews; iOS renderer; store builds | the same fixtures pass on every platform |
| 16 | maps to look at inside the app: a map library and tiles downloaded at will while there is network, from today or the night before; the navigator stays for getting there | a park's points show on a real map with the phone offline |
| 17 | the day rearranged on one page: blocks dragged and dropped, each as tall as it lasts, the travel between two blocks a locked block that changes when they move or swap, not when one is resized | a day is reordered by hand and the travel between follows |
| 18 | Android coverage near 100%: the screens under Compose tests on the JVM, the activity's logic moved into tested code, the Kover exclusions down to the generated bindings | the Kover floor at 95 or more, and every class still left out named with its reason |

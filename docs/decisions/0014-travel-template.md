# 0014: a pack extends a bundled template; travel is the first

Status: accepted, 2026-09-27.

## Context

Every trip pack would repeat the same definition: the same modules, the same rules, the same
screens, word for word. A pack author (or their LLM) should write the trip, not the app. The
design has six views and thirteen moment types for a trip, and they belong to the shell's travel
template, not to each pack.

## Decision

**`pack.extends: <name>` starts the manifest from a template bundled in the engine**
(`templates/<name>/pack.yaml`, compiled in with `include_str!`). The merge:

- `modules`, `derive`, `screens` and `ui` merge by key; the pack wins where both have a key.
- The pack's `rules` come first, the template's after, so a pack adds a case without restating
  the table.
- Any other section is the pack's.

**The template is ordinary pack YAML.** It needs nothing the engine does not offer every pack.
What it needed was added to the engine for everyone:

- the `Auto` component (the unknown key rule, with `skip`);
- literal list props;
- `Card`, `Missing` and `Alert` bodies that follow path references;
- the `[{ui.to_confirm}]` marker;
- timeline `days`, `tomorrow` and `day.started`, and a `state` per block;
- the holder read from the stored `holder` when the host has none.

## Deviations from the design

- **Per-type cards come from the pack's order, through `Auto`.** There is no fixed card order per
  type. The type only picks the answer at the top of a moment.
- **No highlight on the first card**, and **no type guessed from the text**: a block with no
  `type` is time plus text.
- **The map and save-parking buttons wait for phase 7**, when the host tools land.
- **A date the pack does not cover shows the list of days**, not the nearest day's agenda.
- **The kid theme comes in phase 5**, with the theme engine.

## Consequences

- A travel pack is `extends: travel`, its content, a theme, and `ui` labels in its language.
- Changing the template changes every pack that extends it. Its tests run a day through
  every screen, and the Maestro flows do the same on a device.
- A new template is a new file and one line in `TEMPLATES`; the JSON schema lists the names.

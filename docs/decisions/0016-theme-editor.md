# 0016: the design system screen edits one value at a time, through the engine

Status: accepted, 2026-09-27.

## Context

Decision 0006 says a theme edit is validated, contrast included, and written to the loaded theme
file with its comments and order kept. Phase 6 built it, and three things came up that 0006 does
not say.

## Decision

**The engine does the edit.** `edit_theme(manifest, files, path, value)` changes one scalar of the
theme file in its text (`edit::set`), loads the whole pack with it, and returns the new text only
when the pack still loads. A refusal is the loader's own: a contrast failure names the pair and the
ratio. The host only writes the text it is handed. No YAML is written anywhere else.

**One value of a block mapping per edit.** A path names a key per level, like
`themes.plain-light.colors.paper`. A value in a flow mapping (`{size: 64px}`), a list, or a key the
file does not have is refused: those would mean writing YAML, not editing a value. A quoted value
stays quoted; a plain one stays plain while it reads back as the same kind of value.

**The bundled example is edited in an overlay.** Its files are app assets and cannot be written, so
an edit lands in the app's own files, over the asset of the same path. A picked folder is written
in place, with the write permission taken when it was picked.

**The screen:** "Design system" in the shell menu shows the holder's theme: its 23 colors, its type
steps, its sizes, and one of each component drawn with them. A tap on a value opens it in a text
field; Save keeps the dialog open with the reason when the edit is refused.

## Deviations from the design

- **No export yet.** When the folder can no longer be written (its permission lost, or taken read
  only before this phase), the edit is refused with that reason and the advice to open the folder
  again. Fixed in [0023](0023-drift-review.md).
- **Type is edited by size only**, and sizes, radius, border and touch by their own value. Weight,
  line and tracking are shown, not edited.

## Consequences

- A theme with flow mappings can be read but only partly edited; the example is written in block
  style so every value is reachable.
- An edit reloads the pack, so what the screen draws is always what the file says.
- A pack with unrelated errors cannot be edited: it does not load, so no edit can pass.

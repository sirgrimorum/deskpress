# 0006: Theme edits in the app write back to the loaded file

Status: accepted, 2026-09-26.

## Context

The app has a design system section: every token and component of the current theme, live, and
editable. The question was where an edit goes: nowhere (preview only), a local override, or the
theme file itself.

## Decision

An edit is written to the theme file the pack was loaded from. The app can always load a pack
(definition, content and theme), so the file in use is the source of truth, and an edit to it is an
edit to the app.

- On Android the host keeps a persisted permission on the picked files, so it writes to the same
  document, not to a copy nobody sees.
- Every edit is validated before it is written, contrast included. An edit that would make text
  unreadable is refused with the pair and the ratio, never saved.
- Export shares the current pack files, so they can go back to a desk or an LLM.

## Consequences

- The app becomes an editor for one file: the theme. Definition and content stay read only.
- The writer must preserve what it does not change. The YAML writer edits values in place and keeps
  comments and order, which is a requirement on the engine's YAML code.
- If the persisted permission is lost (file moved, storage cleared), the app says so and offers
  export instead of failing silently.

# 0018: documents

Status: accepted, 2026-09-28.

## Context

Phase 7 part 2. At a counter somebody asks for the insurance, a booking, a card: the file has to
open with no signal, bright enough to scan, and the number to call when it goes wrong has to be at
hand before the file. None of it is for a child holding the phone.

## Decision

**Numbers before the file.** A document takes `call`, a mapping of label to number. The module
turns it into `[{label, number}]` and adds `person`, the name of its `for`. The `document` screen
shows one call button per number, then the fields, then the button to the file.

**Two host commands.** `{do: phone.call, with: {number}}` opens the dialer with the number typed
in; the person presses call, so the app needs no call permission. `{do: document.open, with:
{file, title}}` shows the file full screen, at maximum brightness while it shows. On Android it is
`PdfRenderer`, one page image per screen width, read from the picked folder or the bundled pack.
Nothing reaches the network.

**The file stays in the pack.** The validator refuses a `file` that is absolute or climbs out with
`..`, the same rule as any other file the pack names, and a `call` number that is not digits with
the usual grouping.

**Adults only, in the module.** With a child holding the phone the `documents` module returns
none, so no screen, rule or pack can show them to a child by mistake. The agenda's button to them
shows only when there are any.

**A group per person.** The documents screen draws one `Group` per person with their documents
([decision 0019](0019-group.md)), and a document with no `for` as a row below the groups.

## Deviations from the design

- **The agenda, not a small button on every screen.** The way in is a button in the agenda, which
  every screen reaches with Today.

## Consequences

- The host command set is now `device.unlock`, `phone.call` and `document.open`, listed in
  [pack-format](../pack-format.md).
- Only PDF opens. Another kind of file shows the viewer's error text.

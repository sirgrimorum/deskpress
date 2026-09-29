# 0015: the renderer draws from theme tokens; packs come from a folder; facts stay on the device

Status: accepted, 2026-09-27.

## Context

Phase 4 drew every node with Material defaults. A pack's `theme.yaml` was read by the engine and
never reached the screen; the app only ever opened the bundled example; and what an action stored
was lost when the app closed.

## Decision

**The tree says whose theme and whether a child holds the phone** (`theme` and `kid`, tree
version 3). The engine keeps deciding; the renderer keeps drawing.

**The renderer draws from tokens, never from a theme id.** `LoadedPack.theme()` hands the theme
file over as a value; the app picks the holder's theme in the system's mode, reads it into tokens,
and fills every gap from a built-in neutral theme. Material stays only for the scaffold and the
menu.

**The Screen node is the frame:** a top bar with the title, back and the other events, and a
bottom bar from `foot_label`, `foot_time` and `foot`. The travel template moved "what comes next"
there.

**A shell menu on every screen:** "Open a pack…" picks a folder through the storage access
framework, keeps its read permission, and reads only `.yaml`, `.yml` and `.json` files;
"Check the pack" lists the validator's errors or warnings. A fresh install opens the bundled
example.

**Facts persist per pack id** in the app's own files, written after every action as the engine's
`encode_facts` text and read back at load.

## Deviations from the design

- **The hero steps down** to `hero-m` and `hero-s` by the answer's length; the design sizes it by
  hand.
- **Row takes a `time` prop** for its fixed time column, rather than guessing a time out of the
  caption.
- **No flow for the folder picker**: it is system UI, and differs across Android versions.

## Consequences

- A pack that ships no theme still draws, in the neutral theme.
- A tree version the app does not know is refused, as before; this one is 3.
- Removing a pack's folder, or its permission, shows the pack as unreadable, not a crash.

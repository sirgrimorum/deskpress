# CLAUDE.md

The context for this project is in [AGENTS.md](AGENTS.md). Read it before touching code.

Then, in this order: `docs/architecture.md` (the shell), `docs/pack-format.md` (every key the
shell knows), and `content/<pack>/design/40-contrato.md` when there is a pack at hand, because
that file is the exact shape of the object each view receives.

Three reminders that are operational rather than architectural:

- `content/` is in `.gitignore` on purpose. Do not move anything out of it, do not copy its
  contents into a versioned file, and do not put pack data in code, tests or commit messages.
- Files under `content/<pack>/design`, `spec` and `wireframes` are copies of artifacts on
  claude.ai that other sessions keep editing. From this repo you read them, you do not publish
  them. The procedure for bringing them up to date is in `content/<pack>/sources.md`.
- This repo writes in English. A pack keeps its own language, and so do those copies.

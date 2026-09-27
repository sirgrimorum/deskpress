# 0009: Who each document is for, and what stays private

Status: accepted, 2026-09-26.

## Context

The repo is open source and is built largely with AI assistants. Humans and assistants need
different documents: a person wants the shape in a minute, with diagrams; an assistant needs every
rule, stated with authority, in one place. And some notes are the owner's alone.

## Decision

| document | reader | style |
| --- | --- | --- |
| `AGENTS.md` | AI assistants | authoritative and complete: rules, commands, map, patterns. `CLAUDE.md` points to it |
| `README.md` at the root and in each part | humans | short, a mermaid diagram, what it is and how to run it |
| `docs/architecture.md` | humans | the machine in diagrams, little prose |
| `docs/pack-format.md` | both | the reference for every key a pack can use |
| `docs/decisions/` | both | why things are the way they are |
| `docs/roadmap.md` | both | what is done and what is next, without dates or people |
| `private/` | the owner | git ignored: plans with dates, PR drafts, personal notes |
| `content/` | the owner | git ignored: real packs |

Rules:

- Every decision updates `AGENTS.md`, the affected READMEs and docs in the same change.
- Code comments are short and say why, not what. `TODO:` marks a real follow up, and a PR that adds
  one says so; the aim is to leave none.
- Every diagram is mermaid.

## Consequences

- Two documents may say the same thing at different depth. When they disagree, `AGENTS.md` and
  `docs/pack-format.md` win, and the other is fixed in the same change.

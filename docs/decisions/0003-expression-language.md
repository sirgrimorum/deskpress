# 0003: A small expression grammar of our own

Status: accepted, 2026-09-26.

## Context

Guards (`when`, `if`), bindings (`text: block.time`) and templates (`"{block.time} · {place.name}"`)
need a language. The options were readable strings with our own parser, structured YAML in the
JSON Logic style, or an existing language (CEL, JSONata).

## Decision

Readable strings, parsed by the engine into a syntax tree at load. Never `eval`.

```
when: decision.due and not decision.answered
if: count(today.alerts) > 0
text: "{block.time} · {place.name}"
```

The grammar is deliberately small: paths, literals, `and or not`, `== != < <= > >=`, `in`,
parentheses and a short list of pure functions. The exact grammar lives in `docs/pack-format.md`
and grows only when a real pack needs it.

## Why not the others

- **For an LLM and a person**, infix expressions read like the conditions in every language they
  know. JSON Logic is rare in training data and deeply nested, and models slip on its brackets. It
  also takes three to four times the tokens.
- **Speed is equal.** Both are parsed once at load; evaluation walks the same tree.
- **An existing language** adds the only runtime dependency the engine would have, is far larger
  than needed, and its error messages are not ours to shape.

## Consequences

- We own a parser of a few hundred lines and its tests.
- Errors carry the key path and the column: `screens.choose_day.when, col 10: unknown path
  decision.dew, did you mean decision.due`.
- Because the tree is known at load, the validator checks every path, function and action before
  the pack is accepted.

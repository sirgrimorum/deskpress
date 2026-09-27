# 0005: A screen is a tree of the shell's components

Status: accepted, 2026-09-26.

## Context

The first design found that 44 drawn screens were six views with different data. A generic shell
still needs a line between what a pack may draw and what the shell owns. The options were fixed
views a pack only picks, a tree of the shell's components, or free layout primitives.

## Decision

A screen's `layout` is a tree of components from a **closed, themed set** that the shell owns:
`BigValue`, `Card`, `Row`, `Alert`, `Button`, `Segmented`, `Chip`, `Missing` and the rest listed in
`docs/pack-format.md`. A pack binds their props to expressions and their events to actions. It
cannot set a color, a size or a font: those come from the theme.

The six views of the first design become **templates**: ready screens in `templates/travel/` that a
pack can reference or copy.

`Auto` is the component that keeps the unknown key rule: given a value, it expands every key the
pack wrote into a `Card`, an `Alert` or a small sheet, so enriching content needs no layout change.

## Consequences

- One renderer per platform draws every pack, and a theme applies to all of them the same way.
- Validation is exact: every component and prop is known.
- A new component is a shell release on every platform. That is the price of the look staying
  coherent and every theme staying legible.

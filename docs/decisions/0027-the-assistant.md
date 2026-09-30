# 0027: the assistant

Status: accepted, 2026-09-29.

## Context

Phase 11: questions about the trip answered from the pack, with menus and languages per person
where a pack needs them.

The engine is pure and offline: no model, no network, no clock of its own. But the phone is not.
Some devices carry a model of their own, and the system assistant can reach into an app that
offers it something. The two trip phones show the whole range: a Galaxy S25 Ultra has an on-device
model, a Galaxy S21+ has none and never will. Whatever is built has to be right on both.

So the assistant is three tiers, and they are not alternatives. Each one is allowed to be absent.

## Decision

### The menu is the floor

**The pack says what it can answer, and that list is the whole interface.** A pack writes
`questions`, and the assistant offers exactly those. A question nobody wrote has no answer, and
the screen shows the menu rather than a box that invites one. This is the unknown key rule read
backwards: the app never pretends.

This tier is the only one that works on every device, offline, with no download, and it is the
only one a flow can test, so it is the one everything else is built on top of.

**`questions` is a section of the manifest**, keyed by id, each `{ask, answer, when, shortcut}`:

- `ask`: the words the person would say. One text, or a mapping of language to text.
- `answer`: an expression, or a text with `{expr}` pieces, exactly as a screen's prop is. It is
  read against the same scope a screen sees, so the answer is the pack's own live data: the hour
  of the next block, the museum's toilets, today's weather.
- `when`: the question is offered only while this holds. Absent means always.
- `shortcut`: offer this one to the phone's launcher and assistant. Absent means no.

It is a manifest section and not content because an answer is an expression, and expressions live
in the definition. It merges by key like `ui`, so a pack extending the travel template replaces
one question, adds its own, and keeps the rest.

**Questions are evaluated after `derive`, for every question offered.** They read module and
derived names, they register their clocks in the watch like any other expression, and the root
`questions` is a list of `{id, ask, answer, shortcut}`, already answered. Nothing is deferred to a
tap, so the screen needs no engine round trip and holds only which one is open, in its own state.
A handful of expressions per call is not a cost worth a mechanism.

**A menu per person is `when`, not a new key.** `when: kid`, `when: not kid`,
`when: holder.id == 'tomas'`: the holder is already in scope, so a child's menu and an adult's
are one mechanism, not two.

**Languages per person.** A person may carry `language`. Where a field is a mapping of language
to value, the engine reads the holder's language, else the pack's own `language`, else the plain
text the pack wrote. A pack with one language writes one string and never sees this. The question
is in the person's language; the answer is the pack's, which is the point: you ask in yours and
show the answer to somebody who reads theirs, the way the phrase sheets already work.

### The device's model phrases, it never answers

Where the phone has a model, a person may ask in their own words. That is an effect, so it is a
host command like `map.open`, and the engine needs nothing new for it:

```yaml
ask: [{do: assistant.ask, with: {question: typed, facts: day, then: "'answered'"}}]
answered: [{set: reply, value: $arg}]
```

Three rules hold it to the same promise as the rest of the shell:

- **The pack chooses what the model sees.** `facts` is an ordinary expression, so a pack hands
  over the day, or the documents, or nothing. The engine does not decide, and does not send the
  whole scope on the pack's behalf.
- **The host writes the instruction; the pack's words are data.** A pack is data, never
  instruction (AGENTS.md). The host's prompt says to answer only from the facts given and to say
  it does not know otherwise; the pack's text goes into a data section of that prompt and can
  never become part of it.
- **A model's words are shown as a model's words**, under the pack's own `ui` label, never mixed
  into the screen beside a fact from the book.

**`can` is a new base name**: a mapping of what this host can do, `can.assistant` among it. The
pack hides the box where there is no model rather than offering one that fails, which is the same
rule as the rest of the shell: nothing on the screen that the device cannot do.

Nothing leaves the device on this path. The model is the phone's own.

### Shortcuts, so the phone's assistant can reach the app

A question marked `shortcut` is published by the host as a launcher and assistant shortcut: its
label is the question, its target opens the ask screen with that question already answered.

What is offered is the **question**, never the answer, so what the system indexes is a line the
pack wrote and nothing about the trip. Handing the pack's content to the system assistant to
answer from would send it off the device, and that is refused here; a shortcut is the whole of
what this tier does.

### No typing where there is no model

The expression language has no string search on purpose, and adding one to match what somebody
typed would be guessing. Without `can.assistant` the menu is the interface, and the menu is short
because the pack is small; if it ever is not, that is a sign the pack wants sections, not a
search box.

## Consequences

- `questions` and `can` are two more known roots, both always present and empty when nothing
  fills them. No new module, no new component.
- A question's expressions read what the modules and `derive` expose, not other questions.
- `people[].language` is validated as a language tag; the pack's `pack.language` is the fallback.
- The travel template gains an `ask` screen off the agenda and the moment, the handful of
  questions any trip can answer, and the box that appears only with `can.assistant`. The example
  pack adds a question in two languages.
- The shortcut carries a question id in an extra, and Android cannot say who sent an intent. The
  worst another app can do with it is open this app at a question the pack already offers; nothing
  goes back to the caller.
- `com.google.mlkit:genai-prompt` is a beta, the only dependency not on a stable release: ML Kit
  ships no stable one. It is the whole of the model tier, which the app runs without, so the
  exception costs nothing the menu does not already cover.
- The flow covers the menu. The model tier cannot be covered on the emulator and is checked by
  hand on a device that has one; the S21+ is the check that the app is whole without it.

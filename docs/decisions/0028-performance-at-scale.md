# 0028: performance at scale

Status: accepted, 2026-09-29.

## Context

Phase 12, four items the phase 7 review wrote down as suspicions: calendar writes one event at a
time, a picked folder listed file by file, the stored facts crossing to Kotlin on every call, and
no keys on the screen's list items. A trip of several hundred events should sync in about a
second, and a large pack should open and draw as fast as the example.

Three of the four are round trips across a process boundary, which is where Android time actually
goes. The fourth was a guess, so it was measured before deciding.

**The measurement.** A generated pack of 60 days, 480 blocks, 20 places, against the example pack
of one day, calling `screen` through UniFFI on the JVM with a debug build of the engine:

| pack | 0 facts | 50 facts | 500 facts |
| --- | --- | --- | --- |
| example, one day | 400 us | 408 us | 1047 us |
| big, 480 blocks | 802 us | 856 us | 1477 us |

A large pack already draws as fast as the example by any measure that matters: twice the work,
both far inside a frame, and the engine itself answers the big pack in about ten milliseconds
including process start (`deskpress screen`, release build). The stored facts cost about 1.3 us
each per call: 65 us for the fifty facts a trip actually keeps, and 650 us only at five hundred.

## Decision

**Three of the four, and the fourth is dropped with its number written down.**

### The calendar is written in one batch

`apply` walks the plan event by event, and each event costs three trips to the calendar provider:
the row, then the old reminder deleted, then the new one inserted. Several hundred events is well
over a thousand transactions, each one a provider transaction that notifies its observers.

The host builds one `ArrayList<ContentProviderOperation>` instead: a delete per removed row, then
for each event its row and its reminders, the reminder rows pointing at the event's with
`withValueBackReference` where the event is new. One `applyBatch(CalendarContract.AUTHORITY, ops)`
writes them, and its results give the row of each insert, which is what the ledger keeps.

`Rows` stops being three methods and becomes one:

```kotlin
interface Rows {
    /** Writes everything in one go: the rows to remove, then each event at its row or at a new
     *  one. Reports what it settled, and what stopped it if anything did. */
    fun write(calendar: Long, remove: List<Long>, write: List<Pair<Event, Long?>>): Wrote
}
```

`apply` keeps its shape: it decides from the plan and the ledger what to remove and what to write,
calls `write` once, and folds the answer back into the ledger. The fake in the tests counts its
calls, so "one batch per sync" is the assertion, not a stopwatch.

What each batch asks for is decided in plain data, `Op` and `Done`, by `batch`, `next` and `wrote`,
which the tests reach. `ContentRows` is left with the translation into `ContentProviderOperation`
and the trip itself, which need a device, and is excluded from coverage like the other host shells.

**What changes for the person**: a batch is one provider transaction, so a batch that fails writes
nothing. A row the person deleted in their own calendar app is the one case that needs a second
batch: the update affects no rows, and those events are inserted in a second call. Two round trips
at worst, and because the second can fail after the first committed, `write` reports what it
settled instead of only what it finished. The ledger takes exactly that, so a sync that stops half
way is continued rather than repeated.

### A picked folder is listed with one query per directory

`Context.pack` walks the folder through `DocumentFile`, and every child costs a query for its name
and another for whether it is a directory. A pack of a hundred files is a couple of hundred
queries before a single one is read.

The host queries each directory once instead, straight against
`DocumentsContract.buildChildDocumentsUriUsingTree`, with `DOCUMENT_ID`, `DISPLAY_NAME` and
`MIME_TYPE` in the projection: one cursor per directory carries everything the walk needs.
`Context.find`, which opens one file by path, keeps `DocumentFile`: it is one file, not a walk.

### The screen's list items carry a key

The renderer draws the screen's nodes with `items(nodes)`, so Compose keys them by position. When
the day is rearranged, a block dropped or an alert cleared, every node after the change is treated
as a different node: state and scroll position go with it.

The engine gives each node a `key`, its path in the layout, and for a node built by `each` the
item's own identity (`event` or `id`, else its index). The renderer passes it to
`items(nodes, key = { it.key })`. It is a string per node and no change to what is drawn, so the
tree version stays at 4: an older renderer that ignores the field draws exactly what it drew
before.

### The stored facts stay where they are

This was the guess, and the numbers say no. Keeping the facts in the `LoadedPack` behind a mutex
would save 65 us per call on a real trip's fifty facts, on a call that happens when somebody taps,
not on every frame. It would cost the engine a second piece of mutable state beside `Nav`, a
`remember` and a `facts` on the FFI surface, and the host losing the one map it can read back.

Not worth it at the sizes a pack reaches. It is written here with its measurement so the next
review does not have to guess again: if a pack ever keeps facts in the hundreds and a tap feels
slow, the table above says what it would buy and this is the design to pick up.

## Consequences

- A batch is atomic, a sync of two batches is not: the ledger records the rows each batch settled,
  so a failure leaves it consistent with the calendar rather than ahead of or behind it.
- One fake in `CalendarTest` counts batches, and the two-batch protocol is tested as data: the gate
  keeps a sync at one trip, which is the part a stopwatch on my desk cannot keep.
- The folder walk is one query per directory by reading, not by a test: it is `ContentResolver` all
  the way down, so it stays uncovered like the rest of the host's I/O.
- The engine grows a `key` per node, and the fixtures grow it with them.
- Keying a node compares it against the siblings already placed, which is quadratic. Release build:
  100 siblings 111 us, 480 siblings 510 us, 1000 siblings 1.5 ms. A day builds well inside a frame,
  so it stays as it is; a screen of several thousand siblings is where a set would start to pay.
- The world still carries the stored facts, and `World.store` stays on the FFI surface.
- Wall clock on the phone is measured by hand, before and after, with the generated pack: the
  numbers go in the pull request, not in the gate, because a timing test on CI is a flake.

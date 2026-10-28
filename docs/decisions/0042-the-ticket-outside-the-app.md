# 0042: the ticket outside the app

Status: accepted, 2026-10-27. Amends 0033.

## Context

On the trip, every gate meant unlocking the phone, opening the app and finding the moment. The
paper was there; getting to it in a queue was the problem.

## Decision

**The engine lists the tickets, the host shows them.** `tickets(world)` gives each timed block
whose ticket (the block's own, else its place's `during`) has documents with a file: when it
starts showing, when it is over, its title and those papers. It shows from the start of the block
before it that day (else midnight), as the moment screen offers it, until the block ends. Over is
dropped. The CLI's `tickets` prints the same list.

**On the phone, a quiet notification and a shortcut.** While a ticket shows: a notification the
lock screen shows in full, with no sound, and a launcher shortcut, both opening its papers. One
alarm at the next start or end keeps them current, and they come back after a restart, like the
alarms of 0033.

**Over the lock screen, only the papers of a ticket due now.** The viewer is its own activity, not
exported; it reads the file from the pack the ticket came from and opens nothing else. A ticket
no longer due opens nothing.

## Consequences

- Papers come whoever holds them: at the gate the whole group's go through one phone.
- Like the alarms, only the open pack's, and only on the real clock: a clock set by hand shows none.
- The flows run under a frozen clock, so the lock screen is checked by hand.

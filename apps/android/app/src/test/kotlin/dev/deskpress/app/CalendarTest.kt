package dev.deskpress.app

import dev.deskpress.engine.Event
import dev.deskpress.engine.Plan
import java.time.ZoneId
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class CalendarTest {
    private fun event(id: String, print: String = "p", reminder: String = "") =
        Event(id, "2026-04-11T10:00", "2026-04-11T11:00", "", "", id, "", "", reminder, print)

    /** A calendar in memory: rows by number, what was done to them, and how many batches it took. */
    private class Memory(val gone: Set<Long> = emptySet(), val failOn: String = "") : Rows {
        val rows = mutableMapOf<Long, String>()
        var batches = 0
        private var next = 100L

        override fun write(calendar: Long, remove: List<Long>, write: List<Pair<Event, Long?>>): Wrote {
            batches++
            if (write.any { it.first.id == failOn }) return Wrote(failed = IllegalStateException("full"))
            for (row in remove) rows.remove(row)
            val written = write.associate { (event, row) ->
                event.id to
                    if (row != null && row !in gone) {
                        rows[row] = "${event.id} again"
                        row
                    } else {
                        rows[next] = "${event.id}@$calendar"
                        next++
                    }
            }
            return Wrote(written, remove)
        }
    }

    @Test
    fun aDamagedLedgerStartsOverAndAGoodOneComesBackAsItWas() {
        for (text in listOf("", "nothing\t1", "calendar\tx", "calendar\t1\t2")) assertNull(Ledger.decode(text))
        val ledger = Ledger(7, mapOf("b" to Link(2, "y"), "a" to Link(1, "x")))
        assertEquals("calendar\t7\na\t1\tx\nb\t2\ty\n", ledger.encode())
        assertEquals(ledger, Ledger.decode(ledger.encode() + "odd line\nc\tx\ty\n"))
        assertEquals(mapOf("a" to "x", "b" to "y"), ledger.known())
    }

    @Test
    fun timesAreLocalToThePackAndTheReminderIsMinutesBefore() {
        assertEquals(1_775_901_600_000, millis("2026-04-11T10:00", ZoneId.of("UTC")))
        assertEquals(1_775_898_000_000, millis("2026-04-11T10:00", ZoneId.of("Europe/Lisbon")))
        val lisbon = ZoneId.of("Europe/Lisbon")
        assertEquals(lisbon, zoneOr("", lisbon))
        assertEquals(lisbon, zoneOr("Not/AZone", lisbon))
        assertEquals(ZoneId.of("Asia/Tokyo"), zoneOr("Asia/Tokyo", lisbon))
        assertNull(minutes(event("a")))
        assertEquals(90L, minutes(event("a", reminder = "2026-04-11T08:30")))
        val plan = Plan(listOf(event("a")), emptyList(), listOf("b", "c"))
        assertEquals("1 new, 0 changed, 2 removed", summary(plan))
    }

    @Test
    fun onlyTheRowsTheLedgerNamesAreTouchedAndARowDeletedByHandIsWrittenAgain() {
        val memory = Memory(gone = setOf(2))
        memory.rows[1] = "a"
        memory.rows[2] = "b"
        memory.rows[3] = "not ours"
        val ledger = Ledger(7, mapOf("a" to Link(1, "x"), "b" to Link(2, "y"), "d" to Link(4, "z")))
        val plan = Plan(listOf(event("c")), listOf(event("b", "y2")), listOf("a", "gone"))
        val done = apply(plan, ledger, memory)
        assertNull(done.failed)
        // Several hundred events or three, a sync is one trip to the calendar (decision 0028).
        assertEquals(1, memory.batches)
        assertEquals(mapOf(2L to "b", 3L to "not ours", 100L to "c@7", 101L to "b@7"), memory.rows)
        assertEquals(mapOf("b" to Link(101, "y2"), "c" to Link(100, "p"), "d" to Link(4, "z")), done.ledger.links)
        memory.rows[4] = "d"
        apply(Plan(emptyList(), listOf(event("d", "z2")), emptyList()), done.ledger, memory)
        assertEquals("d again", memory.rows[4])
    }

    @Test
    fun aSyncThatFailsWritesNothingAndLeavesTheLedgerAsItWas() {
        val plan = Plan(listOf(event("a"), event("b"), event("c")), emptyList(), listOf("d"))
        val ledger = Ledger(7, mapOf("d" to Link(1, "x")))
        val memory = Memory(failOn = "b")
        memory.rows[1] = "d"
        val done = apply(plan, ledger, memory)
        assertEquals("full", done.failed?.message)
        assertEquals(ledger, done.ledger)
        assertEquals(mapOf(1L to "d"), memory.rows)
    }

    @Test
    fun aSyncThatStopsHalfWayKeepsTheRowsItAlreadyWrote() {
        val half = object : Rows {
            override fun write(calendar: Long, remove: List<Long>, write: List<Pair<Event, Long?>>) =
                Wrote(mapOf("a" to 100L), remove, IllegalStateException("half"))
        }
        val ledger = Ledger(7, mapOf("d" to Link(1, "x")))
        val done = apply(Plan(listOf(event("a"), event("b")), emptyList(), listOf("d")), ledger, half)
        assertEquals("half", done.failed?.message)
        // `a` is in the ledger at the row the first batch gave it, so the next sync does not add it again.
        assertEquals(mapOf("a" to Link(100, "p")), done.ledger.links)
    }

    @Test
    fun theFirstBatchRemovesThenWritesAndWhatItDidNotSettleGoesInASecond() {
        val (a, b, c) = Triple(event("a"), event("b"), event("c"))
        val write = listOf(a to null, b to 2L, c to 3L)
        assertEquals(listOf(Op.Remove(1), Op.Add(a), Op.Change(b, 2), Op.Change(c, 3)), batch(listOf(1L), write))
        // The provider's answer to the writes: a new row for `a`, `b` written over, `c` gone by hand.
        val done = listOf(Done(row = 100), Done(rows = 1), Done(rows = 0))
        val next = next(write, done)
        assertEquals(mapOf("a" to 100L, "b" to 2L), next.rows)
        assertEquals(listOf(c), next.again)
        // Adds first, so `again` lines up with the second batch's answers, then the reminders.
        assertEquals(listOf(Op.Add(c), Op.Remind(b, 2)), next.ops)
        assertEquals(mapOf("c" to 101L), wrote(next.again, listOf(Done(row = 101), Done(rows = 1))))
    }

    @Test
    fun aBatchWithNothingLeftOverNeedsNoSecondOne() {
        val write = listOf(event("a") to 2L)
        val next = next(write, listOf(Done(rows = 1)))
        assertEquals(mapOf("a" to 2L), next.rows)
        assertEquals(listOf(Op.Remind(event("a"), 2)), next.ops)
        assertEquals(emptyList<Event>(), next.again)
        assertEquals(emptyMap<String, Long>(), wrote(emptyList(), emptyList()))
    }
}

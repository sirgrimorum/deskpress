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

    /** A calendar in memory: rows by number, and what was done to them. */
    private class Memory(val gone: Set<Long> = emptySet(), val failOn: String = "", val stuck: Long = 0) : Rows {
        val rows = mutableMapOf<Long, String>()
        var next = 100L

        override fun insert(calendar: Long, event: Event): Long {
            if (event.id == failOn) throw IllegalStateException("full")
            rows[next] = "${event.id}@$calendar"
            return next++
        }

        override fun update(calendar: Long, row: Long, event: Event): Boolean {
            if (row in gone) return false
            rows[row] = "${event.id} again"
            return true
        }

        override fun delete(calendar: Long, row: Long) {
            if (row == stuck) throw IllegalStateException("locked")
            rows.remove(row)
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
        assertEquals(mapOf(2L to "b", 3L to "not ours", 100L to "c@7", 101L to "b@7"), memory.rows)
        assertEquals(mapOf("b" to Link(101, "y2"), "c" to Link(100, "p"), "d" to Link(4, "z")), done.ledger.links)
        memory.rows[4] = "d"
        apply(Plan(emptyList(), listOf(event("d", "z2")), emptyList()), done.ledger, memory)
        assertEquals("d again", memory.rows[4])
    }

    @Test
    fun aSyncThatStopsShortKeepsWhatItWrote() {
        val plan = Plan(listOf(event("a"), event("b"), event("c")), emptyList(), emptyList())
        val done = apply(plan, Ledger(7), Memory(failOn = "b"))
        assertEquals("full", done.failed?.message)
        assertEquals(mapOf("a" to Link(100, "p")), done.ledger.links)
        // A row that could not be deleted stays in the ledger, so the next sync tries it again.
        val ledger = Ledger(7, mapOf("a" to Link(1, "x"), "b" to Link(2, "y")))
        val removed = apply(Plan(emptyList(), emptyList(), listOf("a", "b")), ledger, Memory(stuck = 2))
        assertEquals("locked", removed.failed?.message)
        assertEquals(mapOf("b" to Link(2, "y")), removed.ledger.links)
    }
}

package dev.deskpress.app

import dev.deskpress.engine.Paper
import dev.deskpress.engine.Ticket
import java.time.ZoneId
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class TicketsTest {
    private val lisbon = ZoneId.of("Europe/Lisbon")
    private val hour = 3_600_000L

    @Test
    fun aTicketShowsOnItsOwnClocksElseThePacksUntilItIsOver() {
        val now = millis("2026-04-11T10:00", lisbon)
        val paper = Paper("Entry", "files/entry.pdf")
        val all = listOf(
            Ticket("over", "2026-04-11T08:00", "", "2026-04-11T10:00", "", "Over", listOf(paper)),
            Ticket("here", "2026-04-11T09:00", "", "2026-04-11T12:00", "", "Here", listOf(paper)),
            Ticket("madrid", "2026-04-11T12:00", "Europe/Madrid", "2026-04-11T14:00", "Europe/Madrid", "Madrid", listOf(paper)),
            Ticket("odd", "not a time", "", "2026-04-11T12:00", "", "Odd", listOf(paper)),
        )
        val passes = passes(all, lisbon, now)
        assertEquals(listOf(Pass("here", now - hour, now + 2 * hour, "Here", listOf(paper)), Pass("madrid", now + hour, now + 3 * hour, "Madrid", listOf(paper))), passes)
        assertEquals(listOf("here"), showing(passes, now).map { it.key })
        // It turns when Madrid's starts, then when each stops, then never.
        assertEquals(now + hour, turn(passes, now))
        assertEquals(listOf("here", "madrid"), showing(passes, now + hour).map { it.key })
        assertEquals(now + 2 * hour, turn(passes, now + hour))
        assertNull(turn(passes, now + 3 * hour))
    }

    @Test
    fun passesGoThroughTheirLinesWithTabsNewlinesAndOddFiles() {
        val passes = listOf(
            Pass("2026-04-11.blocks.0", 1_776_000_000_000, 1_776_000_900_000, "Train\tcoach 4", listOf(Paper("Rita's\nseat", "files/a b%.pdf"), Paper("Tomás", "files/t.pdf"))),
            Pass("k", 5, 6, "", emptyList()),
        )
        assertEquals(passes, decodePasses(encodePasses(passes)))
        assertEquals(emptyList<Pass>(), decodePasses(""))
        // A line with no instants, or damaged by hand, is dropped; a paper with no file too.
        assertEquals(List(2) { Pass("k", 7, 8, "", emptyList()) }, decodePasses("bad\nk\t7\t8\nj\t%\t9\nk\t7\t8\t\tlone"))
    }
}

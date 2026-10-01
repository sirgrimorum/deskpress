package dev.deskpress.app

import dev.deskpress.engine.Alarm
import java.time.ZoneId
import org.junit.Assert.assertEquals
import org.junit.Test

class AlarmsTest {
    private val lisbon = ZoneId.of("Europe/Lisbon")

    @Test
    fun anAlarmRingsOnItsOwnClockElseThePacksAndOnlyOnceToCome() {
        val now = millis("2026-04-11T10:00", lisbon)
        val all = listOf(
            Alarm("past", "2026-04-11T09:00", "", "Past", ""),
            Alarm("here", "2026-04-11T11:00", "", "Here", "Go"),
            Alarm("madrid", "2026-04-11T11:00", "Europe/Madrid", "Madrid", ""),
            Alarm("odd", "not a time", "", "Odd", ""),
        )
        // Madrid is an hour ahead, so its 11:00 is Lisbon's 10:00: no longer to come.
        assertEquals(listOf(Ring("here", now + 3_600_000, "Here", "Go")), rings(all, lisbon, now))
        // Soonest first, whatever the clock: Madrid's 11:00 rings before Lisbon's.
        assertEquals(listOf("madrid", "here"), rings(all, lisbon, now - 1).map { it.key })
    }

    @Test
    fun onlyTheSoonestAreArmedAndEachRingArmsTheNextSoNoneIsLost() {
        val all = (0 until 300).map { Alarm("a$it", "2026-04-11T10:00", "", "", "") } +
            Alarm("first", "2026-04-11T09:00", "", "", "")
        val kept = rings(all, lisbon, 0)
        assertEquals(301, kept.size)
        assertEquals("first", kept.first().key)
        val (none, armed) = slide(emptyList(), emptySet(), kept)
        assertEquals(emptyList<Ring>(), none)
        assertEquals(kept.take(MOST_RINGS), armed)
        // The first rings: it leaves the window and the one past it comes in, nothing else is called.
        val (cancel, arm) = slide(kept, armed.toSet(), kept.drop(1))
        assertEquals(listOf("first"), cancel.map { it.key })
        assertEquals(listOf(kept[MOST_RINGS]), arm)
        assertEquals(emptyList<Ring>() to emptyList<Ring>(), slide(kept, armed.toSet(), kept))
    }

    @Test
    fun ringsGoThroughTheirLinesWithTabsNewlinesAndBackslashesInTheirWords() {
        val rings = listOf(
            Ring("alert.tide.2026-04-11T12:00", 1_776_000_000_000, "Tide\tlow", "Move\r\nthe car \\ now 100%"),
            Ring("leave.x", 5, "", ""),
        )
        assertEquals(rings, decodeRings(encodeRings(rings)))
        assertEquals(emptyList<Ring>(), decodeRings(""))
        // A line with no instant, or one damaged by hand, is dropped; a backslash is kept.
        assertEquals(listOf(Ring("k", 7, "a\\", "")), decodeRings("bad\nk\t7\ta\\\nj\t8\t%"))
    }
}

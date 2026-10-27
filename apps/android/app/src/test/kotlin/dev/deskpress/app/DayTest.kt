package dev.deskpress.app

import dev.deskpress.engine.Value
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class DayTest {
    /** Where each hour is drawn, as `row left+width` in hundredths of the Day's width. */
    private fun drawn(hours: List<Hour>, tracks: Int) =
        spots(hours, tracks).map { "${it.hour.row} ${(it.left * 100).toInt()}+${(it.width * 100).toInt()}" }

    @Test
    fun anHourReadsAsMinutesFromMidnight() {
        assertEquals(615, clock("10:15"))
        for (bad in listOf("", "10", "ten:15", "10:15:00", "false")) assertNull(bad, clock(bad))
    }

    @Test
    fun theRulerRunsFromTheFirstHourToTheHourAfterTheLastEnds() {
        val day = listOf(Hour(0, 555, 60, null, false), Hour(1, 700, 5, null, false), Hour(2, 650, 20, null, true))
        assertEquals(540..720, axis(day))
        // A block is never drawn shorter than a quarter of an hour; a way is as long as it is.
        assertEquals(715, day[1].end)
        assertEquals(670, day[2].end)
        assertEquals(540..600, axis(listOf(Hour(0, 540, 60, null, false))))
    }

    @Test
    fun eachTrackIsAColumnAndEveryonesHoursCrossThemAll() {
        val day =
            listOf(
                Hour(0, 540, 60, null, false),
                Hour(1, 600, 120, 0, false),
                Hour(2, 600, 75, 1, false),
                Hour(3, 590, 10, 1, true),
                Hour(4, 720, 60, null, false),
            )
        assertEquals(listOf("0 0+100", "1 0+50", "2 50+50", "3 50+50", "4 0+100"), drawn(day, 2))
        // One track, or none: no columns.
        assertEquals(listOf("0 0+100", "1 0+100"), drawn(day.take(2), 1))
    }

    @Test
    fun hoursThatOverlapShareTheirColumnInLanes() {
        val day =
            listOf(
                Hour(0, 600, 60, null, false),
                Hour(1, 630, 60, null, false),
                Hour(2, 660, 60, null, false),
                Hour(3, 720, 30, null, false),
            )
        // The first and the third take turns in one lane; the fourth starts a run of its own.
        assertEquals(listOf("0 0+50", "1 50+50", "2 0+50", "3 0+100"), drawn(day, 0))
        assertEquals(listOf("0 0+25", "1 25+25"), drawn(day.take(2).map { it.copy(track = 0) }, 2))
    }

    @Test
    fun aDragIsReadInStepsOfFifteenMinutes() {
        assertEquals(0, step(7f))
        assertEquals(15, step(8f))
        assertEquals(-30, step(-29f))
        assertEquals(15, step(7.5f))
    }

    @Test
    fun aDragLandsTheWholeBlockOrMovesOneEdge() {
        val h = Hour(0, 615, 45, null, false)
        assertEquals(600 to 645, landing(h, null, -15))
        assertEquals(570 to 660, landing(h, "start", -45))
        assertEquals(615 to 645, landing(h, "end", -15))
    }

    @Test
    fun aDropSaysWhichHourByHowMuchAndForAResizeWhichEdge() {
        val move = dragged(Value.Text("d.blocks.1"), 30)
        assertEquals("d.blocks.1", move.field("block").text())
        assertEquals(Value.Number(30.0), move.field("by"))
        assertNull(move.field("edge"))
        assertEquals("end", dragged(Value.Text("d.blocks.1"), -15, "end").field("edge").text())
    }
}

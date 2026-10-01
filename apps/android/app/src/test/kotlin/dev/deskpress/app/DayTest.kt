package dev.deskpress.app

import dev.deskpress.engine.Value
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class DayTest {
    // 09:00 to 10:00 in 100 px, then 10:15 to 11:00 in 60 px: a quarter hour of free time between.
    private val spans = listOf(Span(0f, 100f, 540, 60), Span(100f, 60f, 615, 45))

    @Test
    fun anHourReadsAsMinutesFromMidnight() {
        assertEquals(615, clock("10:15"))
        for (bad in listOf("", "10", "ten:15", "10:15:00")) assertNull(bad, clock(bad))
    }

    @Test
    fun aPointReadsAsTheMinuteOfTheRowUnderIt() {
        assertEquals(570f, minuteAt(spans, 50f), 0.01f)
        assertEquals(615f, minuteAt(spans, 100f), 0.01f)
        assertEquals(645f, minuteAt(spans, 140f), 0.01f)
        // Past either end, the end row's pace.
        assertEquals(510f, minuteAt(spans, -50f), 0.01f)
        assertEquals(705f, minuteAt(spans, 220f), 0.01f)
    }

    @Test
    fun aMinuteIsDrawnWhereItReadsAndOneBetweenRowsAtTheNextTop() {
        assertEquals(50f, yAt(spans, 570f), 0.01f)
        assertEquals(100f, yAt(spans, 605f), 0.01f)
        assertEquals(140f, yAt(spans, 645f), 0.01f)
        assertEquals(-50f, yAt(spans, 510f), 0.01f)
        assertEquals(220f, yAt(spans, 705f), 0.01f)
        // A row of no minutes reads as one, never a division by zero.
        assertEquals(10f, yAt(listOf(Span(0f, 10f, 540, 0)), 541f), 0.01f)
    }

    @Test
    fun aDragIsReadInStepsOfFifteenMinutes() {
        assertEquals(0, step(7f))
        assertEquals(15, step(8f))
        assertEquals(-30, step(-29f))
        assertEquals(15, step(7.5f))
    }

    @Test
    fun aDragReadsFromTheEdgeItHolds() {
        // The first row's foot is 10:00, though the next row's top reads 10:15.
        assertEquals(0, draggedBy(spans, spans[0], "end", 95f))
        assertEquals(-15, draggedBy(spans, spans[0], "end", 75f))
        assertEquals(15, draggedBy(spans, spans[0], "end", 100f))
        assertEquals(-15, draggedBy(spans, spans[1], null, 95f))
        assertEquals(30, draggedBy(spans, spans[1], "start", 140f))
    }

    @Test
    fun aDragLandsTheWholeBlockOrMovesOneEdge() {
        val second = spans[1]
        // Fifteen minutes earlier: into the free quarter hour, at the first row's foot, same height.
        assertEquals(100f to 160f, landing(spans, second, null, -15))
        assertEquals(50f to 160f, landing(spans, second, "start", -45))
        assertEquals(100f to 140f, landing(spans, second, "end", -15))
        // Past either end of the Day, at the end row's pace.
        assertEquals(-50f to 100f, landing(spans, spans[0], "start", -30))
        assertEquals(100f to 220f, landing(spans, second, "end", 45))
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

package dev.deskpress.app

import dev.deskpress.engine.Field
import dev.deskpress.engine.Value
import org.junit.Assert.assertEquals
import org.junit.Test

class DayTest {
    private fun entry(vararg fields: Pair<String, Value>) = Value.Fields(fields.map { Field(it.first, it.second) })

    @Test
    fun theHoursOfADayAreItsTimedEntriesButTheWayBetweenThem() {
        val items = listOf(
            entry("text" to Value.Text("A note")),
            entry("time" to Value.Text("09:00")),
            entry("time" to Value.Text("09:40"), "leg" to Value.Bool(true)),
            entry("time" to Value.Text("10:00"), "leg" to Value.Bool(false)),
        )
        assertEquals(listOf(1, 3), hours(items))
    }

    @Test
    fun aDraggedHourLandsPastTheCentresItCrossesAndStopsBeforeALockedOne() {
        val centres = listOf(50f, 150f, 250f, 350f, 450f)
        val locked = listOf(false, false, false, true, false)
        assertEquals(0, landing(centres, locked, 0, 40f))
        assertEquals(1, landing(centres, locked, 0, 110f))
        assertEquals(2, landing(centres, locked, 0, 1000f))
        assertEquals(2, landing(centres, locked, 2, 0f))
        assertEquals(0, landing(centres, locked, 2, -1000f))
        assertEquals(1, landing(centres, locked, 2, -150f))
        assertEquals(4, landing(centres, locked, 4, -1000f))
    }

    @Test
    fun aDropSaysWhichHourAndWhereTo() {
        val drop = moved(Value.Text("d.blocks.1"), 3)
        assertEquals("d.blocks.1", drop.field("block").text())
        assertEquals(Value.Number(3.0), drop.field("to"))
    }
}

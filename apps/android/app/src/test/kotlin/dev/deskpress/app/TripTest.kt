package dev.deskpress.app

import dev.deskpress.engine.Taken
import dev.deskpress.engine.Value
import org.junit.Assert.assertEquals
import org.junit.Test

class TripTest {
    @Test
    fun whatATripBringsIsSaidInALine() {
        val one = Taken(mapOf("car" to Value.Null), emptyMap(), emptyMap(), "")
        assertEquals("1 newer fact from the other phone.", brings(one))
        val pack = Taken(emptyMap(), emptyMap(), mapOf("pack.yaml" to ""), "2026-04-10T09:00")
        assertEquals("The pack as updated 2026-04-10 09:00.", brings(pack))
        val both = Taken(mapOf("a" to Value.Null, "b" to Value.Null), emptyMap(), mapOf("pack.yaml" to ""), "2026-04-10")
        assertEquals("2 newer facts from the other phone, and the pack as updated 2026-04-10.", brings(both))
        assertEquals("Nothing there is newer than on this phone.", brings(Taken(emptyMap(), emptyMap(), emptyMap(), "")))
    }
}

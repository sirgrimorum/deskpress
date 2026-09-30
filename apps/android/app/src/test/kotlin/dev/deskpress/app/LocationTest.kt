package dev.deskpress.app

import dev.deskpress.engine.Region
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class LocationTest {
    @Test
    fun aDegreeOfLatitudeIsAboutOneHundredElevenKilometres() {
        assertEquals(111_195.0, distance(0.0, 0.0, 1.0, 0.0), 1.0)
        assertEquals(0.0, distance(10.0, 20.0, 10.0, 20.0), 0.0)
    }

    @Test
    fun aDistanceIsSaidInTensOfMetresOrTenthsOfAKilometre() {
        assertEquals("120 m", metres(123.0))
        assertEquals("1.2 km", metres(1234.0))
        // What would round to 1000 m is said in kilometres.
        assertEquals("990 m", metres(994.0))
        assertEquals("1.0 km", metres(995.0))
    }

    @Test
    fun theRegionsAroundAPointComeDeepestFirstAndTheOnesAwayAreLeftOut() {
        val town = Region("town", 0.0, 0.0, 5000.0)
        val square = Region("square", 0.0, 0.001, 100.0)
        val far = Region("far", 1.0, 1.0, 100.0)
        val regions = listOf(town, square, far)
        assertEquals(listOf("square", "town"), inside(regions, 0.0, 0.0011))
        assertEquals(listOf("town"), inside(regions, 0.0, -0.01))
        assertEquals(emptyList<String>(), inside(regions, 5.0, 5.0))
        // A point exactly on the edge is inside.
        val edge = Region("edge", 0.0, 0.0, distance(0.0, 0.0, 0.0, 0.001))
        assertEquals(listOf("edge"), inside(listOf(edge), 0.0, 0.001))
    }

    @Test
    fun theDaysStopsGoToGoogleMapsInOrder() {
        val (a, b, c) = listOf(Stop(1.5, 2.0), Stop(3.0, 4.0), Stop(5.0, 6.0))
        val dir = "https://www.google.com/maps/dir/?api=1"
        assertEquals("$dir&origin=1.5%2C2.0&destination=5.0%2C6.0&waypoints=3.0%2C4.0", directions(listOf(a, b, c)))
        assertEquals("$dir&origin=1.5%2C2.0&destination=3.0%2C4.0", directions(listOf(a, b)))
        assertEquals("$dir&destination=1.5%2C2.0", directions(listOf(a)))
        assertTrue(directions(listOf(a, b, c, a)).endsWith("&waypoints=3.0%2C4.0%7C5.0%2C6.0"))
        assertEquals("$dir&destination=0.00050%2C-9.1", directions(listOf(Stop(0.0005, -9.1))))
        // Nine waypoints at most, and the day still ends where it ends.
        val long = directions(List(12) { a } + c)
        assertEquals(9, long.substringAfter("&waypoints=").split("%7C").size)
        assertTrue(long.contains("&destination=5.0%2C6.0&"))
    }

    @Test
    fun theStripSaysWhatIsPretendAndNamesThePlace() {
        val museum = Region("museum", 38.7, -9.1, 100.0)
        assertEquals("", pretend(Shell(), listOf(museum)))
        assertEquals(null, "38.7".pinned())
        assertEquals(null, "1,2,3".pinned())
        assertEquals(null, "".pinned())
        assertEquals(38.7 to -9.1, "38.7,-9.1".pinned())
        assertEquals("Pretending: at museum.", pretend(Shell(at = "38.7,-9.1"), listOf(museum)))
        assertEquals(
            "Pretending: the clock is set by hand, at 1.0,2.0.",
            pretend(Shell(shift = 3600, at = "1.0,2.0"), listOf(museum)),
        )
    }
}

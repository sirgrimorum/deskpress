package dev.deskpress.app

import dev.deskpress.engine.Region
import org.junit.Assert.assertEquals
import org.junit.Test

class LocationTest {
    @Test
    fun aDegreeOfLatitudeIsAboutOneHundredElevenKilometres() {
        assertEquals(111_195.0, distance(0.0, 0.0, 1.0, 0.0), 1.0)
        assertEquals(0.0, distance(10.0, 20.0, 10.0, 20.0), 0.0)
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
}

package dev.deskpress.app

import dev.deskpress.engine.Field
import dev.deskpress.engine.Node
import dev.deskpress.engine.Region
import dev.deskpress.engine.Value
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class MapsTest {
    @Test
    fun aSmallPlaceIsKeptAKilometreAroundInTheCellItSitsIn() {
        val cloister = Region("cloister", 38.7245, -9.1141, 100.0)
        assertEquals(setOf(Cell(1944, 1569)), cells(listOf(cloister)))
        val (w, s, e, n) = Cell(1944, 1569).bounds()
        assertEquals(-9.140625, w, 1e-9)
        assertEquals(-9.052734375, e, 1e-9)
        assertEquals(38.6855, s, 1e-4)
        assertEquals(38.7541, n, 1e-4)
    }

    @Test
    fun aPlaceNearACornerTakesTheCellsItsCircleTouchesAndNotTheCorner() {
        // 800 m from the north and west edges, so 1131 m from the corner between them.
        val near = Region("near", 38.746897, -9.131411, 10.0)
        assertEquals(setOf(Cell(1944, 1569), Cell(1944, 1568), Cell(1943, 1569)), cells(listOf(near)))
        // A wide place is kept as wide as it is, and two places share their cells.
        val town = Region("town", 38.7245, -9.1141, 20_000.0)
        assertEquals(33, cells(listOf(town, near)).size)
        assertEquals(emptySet<Cell>(), cells(emptyList()))
    }

    @Test
    fun aKeepAsksForTheCellAHairInsideAndPlainPaperFetchesNothing() {
        val cell = Cell(1944, 1569)
        val (w, s, e, n) = cell.bounds()
        val corners = cell.inset()
        assertEquals(corners.first(), corners.last())
        corners.forEach { (lon, lat) -> assertTrue(lon > w && lon < e && lat > s && lat < n) }
        assertEquals(
            """{"version":8,"sources":{},"layers":[{"id":"paper","type":"background","paint":{"background-color":"#F4EFE6"}}]}""",
            paper(0xFFF4EFE6.toInt()),
        )
    }

    @Test
    fun theKeptCellsGoIntoTheSettingsAndBack() {
        val kept = setOf(Cell(1944, 1569), Cell(1943, 1568))
        assertEquals("1944,1569;1943,1568", kept.encode())
        assertEquals(kept, kept.encode().cells())
        assertEquals(emptySet<Cell>(), "".cells())
        assertEquals(setOf(Cell(1, 2)), "1,2;x,3;4".cells())
    }

    @Test
    fun settingsSaysWhatIsKeptAndWhatAKeepWouldTake() {
        val one = setOf(Cell(1, 1))
        val two = setOf(Cell(1, 1), Cell(1, 2))
        assertEquals("Keeping the maps: 42%.", offline(emptySet(), 0, two, 0.42f))
        assertEquals("Not kept. About 3 MB for the trip's places.", offline(emptySet(), 0, two, null))
        assertEquals("Kept for every place: 18 MB.", offline(two, 18_200_000, one, null))
        assertEquals("Kept for every place: 1 MB.", offline(one, 1_000, one, null))
        assertEquals("Kept, but some places are new since. Keeping again takes about 3 MB.", offline(one, 1_500_000, two, null))
        assertEquals("No place in the pack has a position, so there is no map to keep.", offline(emptySet(), 0, emptySet(), null))
    }

    @Test
    fun aPointWithAPositionIsAPinAndTheMapSaysHowFarItIs() {
        val gate = point(38.7245, -9.1141).let { Value.Fields((it as Value.Fields).fields + Field("name", Value.Text("Gate"))) }
        val pin = pin(gate)
        assertEquals(Pin("Gate", 38.7245, -9.1141), pin)
        assertNull(pin(Value.Fields(listOf(Field("lat", Value.Number(1.0))))))
        assertNull(pin(Value.Text("x")))
        // Off the globe, or not a number at all: no pin, rather than a map that throws.
        assertNull(pin(point(91.0, 0.0)))
        assertNull(pin(point(0.0, Double.POSITIVE_INFINITY)))
        assertNull(pin(point(Double.NaN, 0.0)))
        // A place the day passes twice is one chip; two gates that share a name are two.
        val other = Pin("Gate", 38.0, -9.0)
        assertEquals(listOf(pin, other), choices(listOf(pin!!, Pin("", 1.0, 2.0), pin, other)))
        assertEquals("Pick", toward(null, 1.0 to 2.0, "Pick", "nowhere"))
        assertEquals("Gate: nowhere.", toward(pin, null, "Pick", "nowhere"))
        assertEquals("Gate: 110 m.", toward(pin, 38.7255 to -9.1141, "Pick", "nowhere"))
    }

    @Test
    fun theFullScreenMapSaysThePacksWordsOrEnglish() {
        val card = Node("0", "Map", mapOf("open" to Value.Text("Guianos")), emptyMap(), emptyList())
        assertEquals("Guianos", word(card, "open"))
        assertEquals("Follow me", word(card, "follow"))
    }
}

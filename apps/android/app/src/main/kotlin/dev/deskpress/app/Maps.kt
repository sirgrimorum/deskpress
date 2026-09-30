package dev.deskpress.app

import dev.deskpress.engine.Node
import dev.deskpress.engine.Region
import dev.deskpress.engine.Value
import kotlin.math.PI
import kotlin.math.atan
import kotlin.math.cos
import kotlin.math.floor
import kotlin.math.ln
import kotlin.math.roundToInt
import kotlin.math.sinh
import kotlin.math.tan

/** The zoom whose tiles are the cells kept offline, about seven kilometres across (decision 0031). */
private const val CELL_ZOOM = 12

/** How far around a place its map is kept at the least, in metres, as the validator warns. */
private const val KEPT_M = 1000.0

/** About what a cell kept at zooms 10 to 14 takes, for the size stated before a keep. */
private const val CELL_BYTES = 1_500_000L

private const val DEGREE_M = 111_320.0

private const val TILES = 1 shl CELL_ZOOM

/** One z12 tile of the map, the unit a keep asks for. */
data class Cell(val x: Int, val y: Int) {
    /** Its edges in degrees: west, south, east, north. */
    fun bounds(): List<Double> = listOf(lon(x), lat(y + 1), lon(x + 1), lat(y))

    /** Its corners as (lon, lat), closed, a hair inside: its edges would take in the tiles next to it. */
    fun inset(): List<Pair<Double, Double>> {
        val (w, s, e, n) = bounds()
        val d = (e - w) / 1000
        return listOf(w + d to s + d, e - d to s + d, e - d to n - d, w + d to n - d, w + d to s + d)
    }
}

private fun lon(x: Int) = x * 360.0 / TILES - 180.0

private fun lat(y: Int) = Math.toDegrees(atan(sinh(PI * (1 - 2.0 * y / TILES))))

private fun column(lon: Double) = floor((lon + 180) / 360 * TILES).toInt().coerceIn(0, TILES - 1)

private fun row(lat: Double): Int {
    val r = Math.toRadians(lat.coerceIn(-85.0511, 85.0511))
    return floor((1 - ln(tan(r) + 1 / cos(r)) / PI) / 2 * TILES).toInt().coerceIn(0, TILES - 1)
}

/** The cells each region's circle touches, the circle padded to a kilometre at least. */
fun cells(regions: List<Region>): Set<Cell> = buildSet {
    for (r in regions) {
        val m = maxOf(r.radiusM, KEPT_M)
        val dlat = m / DEGREE_M
        val dlon = dlat / cos(Math.toRadians(r.lat))
        for (x in column(r.lon - dlon)..column(r.lon + dlon)) {
            for (y in row(r.lat + dlat)..row(r.lat - dlat)) {
                val (w, s, e, n) = Cell(x, y).bounds()
                // The cell's nearest point to the centre: a corner of the square around it may miss.
                if (distance(r.lat, r.lon, r.lat.coerceIn(s, n), r.lon.coerceIn(w, e)) <= m) add(Cell(x, y))
            }
        }
    }
}

/** Cells as the shell's preferences keep them, "x,y" apart by ";". */
fun Set<Cell>.encode(): String = joinToString(";") { "${it.x},${it.y}" }

/** The cells `encode` wrote; a malformed entry is skipped. */
fun String.cells(): Set<Cell> =
    split(";").mapNotNull { c -> c.split(",").mapNotNull(String::toIntOrNull).takeIf { it.size == 2 }?.let { Cell(it[0], it[1]) } }.toSet()

/** What Settings says of the maps kept, against the cells the pack's places need now. */
fun offline(kept: Set<Cell>, bytes: Long, wanted: Set<Cell>, progress: Float?): String =
    when {
        progress != null -> "Keeping the maps: ${(progress * 100).roundToInt()}%."
        kept.isNotEmpty() && kept.containsAll(wanted) -> "Kept for every place: ${megabytes(bytes)}."
        wanted.isEmpty() -> "No place in the pack has a position, so there is no map to keep."
        kept.isEmpty() -> "Not kept. About ${megabytes(wanted.size * CELL_BYTES)} for the trip's places."
        else -> "Kept, but some places are new since. Keeping again takes about ${megabytes(wanted.size * CELL_BYTES)}."
    }

private fun megabytes(bytes: Long) = "${(bytes / 1e6).roundToInt().coerceAtLeast(1)} MB"

/** A style of plain paper, with no source to fetch from, for the map where nothing is kept. */
fun paper(argb: Int): String =
    """{"version":8,"sources":{},"layers":[{"id":"paper","type":"background","paint":{"background-color":"#%06X"}}]}""".format(argb and 0xFFFFFF)

/** A point of a Map card that has a position, as the full-screen map draws it. */
data class Pin(val name: String, val lat: Double, val lon: Double)

/** `point` as a pin, or null when it carries no position on the globe: the map library throws on one. */
fun pin(point: Value): Pin? {
    val lat = (point.field("lat") as? Value.Number)?.value?.takeIf { it in -90.0..90.0 } ?: return null
    val lon = (point.field("lon") as? Value.Number)?.value?.takeIf { it in -180.0..180.0 } ?: return null
    return Pin(point.field("name").text(), lat, lon)
}

/** A Map card's word for the full-screen map, in the pack's language, or English when it gives none. */
fun word(node: Node, key: String): String = node.props[key].text().ifEmpty { ENGLISH.getValue(key) }

private val ENGLISH = mapOf("open" to "Lead the way", "pick" to "Pick where to go next.", "follow" to "Follow me", "unplaced" to "no position yet")

/** The pins to pick from under the map: each once, and only those with a name to show. */
fun choices(pins: List<Pin>): List<Pin> = pins.filter { it.name.isNotEmpty() }.distinct()

/** What the full-screen map says under it: how far the pin picked is from here. */
fun toward(pin: Pin?, here: Pair<Double, Double>?, pick: String, unplaced: String): String =
    when {
        pin == null -> pick
        here == null -> "${pin.name}: $unplaced."
        else -> "${pin.name}: ${metres(distance(here.first, here.second, pin.lat, pin.lon))}."
    }

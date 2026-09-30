package dev.deskpress.app

import dev.deskpress.engine.Region
import java.math.BigDecimal
import kotlin.math.asin
import kotlin.math.cos
import kotlin.math.pow
import kotlin.math.roundToInt
import kotlin.math.sin
import kotlin.math.sqrt

private const val EARTH_M = 6_371_000.0

/** Metres between two points on the earth, along its surface. */
fun distance(lat1: Double, lon1: Double, lat2: Double, lon2: Double): Double {
    val (p1, p2) = Math.toRadians(lat1) to Math.toRadians(lat2)
    val dp = p2 - p1
    val dl = Math.toRadians(lon2 - lon1)
    val a = sin(dp / 2).pow(2) + cos(p1) * cos(p2) * sin(dl / 2).pow(2)
    return 2 * EARTH_M * asin(sqrt(a))
}

/** A distance rounded to what the eye can use: whole tens of metres, or tenths of a kilometre. */
fun metres(m: Double): String =
    if (m < 995.0) "${(m / 10.0).roundToInt() * 10} m" else "${(m / 100.0).roundToInt() / 10.0} km"

/**
 * The ids of the regions around `lat`, `lon`, the smallest first: a place inside a larger one
 * comes before it.
 */
fun inside(regions: List<Region>, lat: Double, lon: Double): List<String> =
    regions.filter { distance(lat, lon, it.lat, it.lon) <= it.radiusM }.sortedBy { it.radiusM }.map { it.id }

/** A pinned "lat,lon" as the two numbers it holds, or null when the position is the real one. */
fun String.pinned(): Pair<Double, Double>? =
    split(",").mapNotNull(String::toDoubleOrNull).takeIf { it.size == 2 }?.let { it[0] to it[1] }

/** What the strip over the pack says while the clock or the position is pretend; empty when neither is. */
fun pretend(shell: Shell, regions: List<Region>): String {
    val where = shell.at.pinned()?.let { (lat, lon) -> regions.firstOrNull { it.lat == lat && it.lon == lon }?.id ?: "$lat,$lon" }
    val parts = listOfNotNull("the clock is set by hand".takeIf { shell.shift != 0L }, where?.let { "at $it" })
    return if (parts.isEmpty()) "" else "Pretending: ${parts.joinToString(", ")}."
}

/** A coordinate as a link carries it: `0.00050`, never `5.0E-4`. */
fun decimal(v: Double): String = BigDecimal.valueOf(v).toPlainString()

/** The stops in order as a Google Maps directions link. One stop is a destination from here. */
fun directions(stops: List<Stop>): String {
    val at = { s: Stop -> "${decimal(s.lat)}%2C${decimal(s.lon)}" }
    val from = if (stops.size > 1) "&origin=${at(stops.first())}" else ""
    // Google takes nine waypoints at most.
    val between = stops.drop(1).dropLast(1).take(9)
    val via = if (between.isEmpty()) "" else "&waypoints=" + between.joinToString("%7C", transform = at)
    return "https://www.google.com/maps/dir/?api=1$from&destination=${at(stops.last())}$via"
}

package dev.deskpress.app

import dev.deskpress.engine.Region
import kotlin.math.asin
import kotlin.math.cos
import kotlin.math.pow
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

/**
 * The ids of the regions around `lat`, `lon`, the smallest first: a place inside a larger one
 * comes before it.
 */
fun inside(regions: List<Region>, lat: Double, lon: Double): List<String> =
    regions.filter { distance(lat, lon, it.lat, it.lon) <= it.radiusM }.sortedBy { it.radiusM }.map { it.id }

package dev.deskpress.app

import dev.deskpress.engine.Field
import dev.deskpress.engine.Value
import kotlin.math.roundToInt

/** A Day's one scale: every minute this many dp, whatever the day holds. */
const val DP_A_MINUTE = 1.6f

/** The fewest minutes a block is drawn as, so a short one still takes a finger. */
const val SHORTEST = 15

/** A timed entry of a Day: its row in the list, its minutes, its track (none: everyone's) and whether it is a way. */
data class Hour(val row: Int, val start: Int, val lasts: Int, val track: Int?, val leg: Boolean) {
    /** Where its drawing ends: a block never shorter than [SHORTEST], a way as long as it is. */
    val end: Int get() = start + if (leg) lasts else maxOf(lasts, SHORTEST)
}

/** Where an hour is drawn across the Day: from `left`, `width` wide, both shares of the width. */
data class Spot(val hour: Hour, val left: Float, val width: Float)

/** Minutes from midnight to an `HH:MM`, or null when it is not one. */
fun clock(time: String): Int? {
    val (h, m) = time.split(":").takeIf { it.size == 2 }?.map { it.toIntOrNull() ?: return null } ?: return null
    return h * 60 + m
}

/** The minutes the ruler shows: from the hour the first begins to the hour after the last ends. */
fun axis(hours: List<Hour>): IntRange {
    val first = hours.minOf { it.start } / 60 * 60
    val last = hours.maxOf { it.end }
    return first..(last + 59) / 60 * 60
}

/**
 * A column per track and everyone's across them all; hours that overlap share their column in lanes.
 * A way takes its column whole: it is drawn behind.
 */
fun spots(hours: List<Hour>, tracks: Int): List<Spot> {
    val columns = maxOf(tracks, 1)
    fun spot(h: Hour, lane: Int, lanes: Int): Spot {
        val (col, span) = h.track?.takeIf { columns > 1 }?.coerceIn(0, columns - 1)?.let { it to 1 } ?: (0 to columns)
        return Spot(h, (col + span * lane / lanes.toFloat()) / columns, span / (lanes.toFloat() * columns))
    }
    val out = hours.filter { it.leg }.map { spot(it, 0, 1) }.toMutableList()
    for ((_, group) in hours.filter { !it.leg }.groupBy { it.track?.takeIf { columns > 1 } }) {
        // Greedy lanes, counted per run of hours that overlap one another.
        var run = mutableListOf<Pair<Hour, Int>>()
        val ends = mutableListOf<Int>()
        fun close() {
            run.forEach { (h, lane) -> out += spot(h, lane, ends.size) }
            run = mutableListOf()
            ends.clear()
        }
        for (h in group.sortedBy { it.start }) {
            if (ends.isNotEmpty() && h.start >= ends.max()) close()
            val lane = ends.indexOfFirst { it <= h.start }.takeIf { it >= 0 } ?: ends.size.also { ends += 0 }
            ends[lane] = h.end
            run += h to lane
        }
        close()
    }
    return out.sortedBy { it.hour.row }
}

/** Minutes as a drag reads them: the nearest fifteen. */
fun step(minutes: Float): Int = (minutes / 15f).roundToInt() * 15

/** The minutes a drag of `by` puts an hour at, start and end: the whole block, or one edge. */
fun landing(h: Hour, edge: String?, by: Int): Pair<Int, Int> =
    when (edge) {
        "start" -> h.start + by to h.end
        "end" -> h.start to h.end + by
        else -> h.start + by to h.end + by
    }

/** What a drop tells the engine: the hour, which edge for a resize, and by how many minutes. */
fun dragged(event: Value, by: Int, edge: String? = null): Value =
    Value.Fields(
        listOfNotNull(
            Field("block", event),
            edge?.let { Field("edge", Value.Text(it)) },
            Field("by", Value.Number(by.toDouble())),
        ),
    )

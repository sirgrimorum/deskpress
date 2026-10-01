package dev.deskpress.app

import dev.deskpress.engine.Field
import dev.deskpress.engine.Value
import kotlin.math.roundToInt

/** A timed row of a Day as the time axis reads it: where it is drawn, in px, and the minutes it covers. */
data class Span(val top: Float, val height: Float, val start: Int, val lasts: Int)

/** Minutes from midnight to an `HH:MM`, or null when it is not one. */
fun clock(time: String): Int? {
    val (h, m) = time.split(":").takeIf { it.size == 2 }?.map { it.toIntOrNull() ?: return null } ?: return null
    return h * 60 + m
}

/** The row a point or a minute falls in: the first that ends after it, else the last. */
private fun List<Span>.at(past: (Span) -> Boolean): Span = firstOrNull { !past(it) } ?: last()

/** How far into a row: from its top, except the first row reads before itself too. */
private fun List<Span>.into(s: Span, by: Float): Float = if (s === first()) by else maxOf(by, 0f)

/** The minute a point of the Day reads as: the row under it, at its pace; past either end, the end row's pace. */
fun minuteAt(spans: List<Span>, y: Float): Float {
    val s = spans.at { y >= it.top + it.height }
    return s.start + spans.into(s, y - s.top) / s.height * maxOf(s.lasts, 1)
}

/** Where a minute is drawn: the inverse of [minuteAt]. A minute between two rows sits at the later one's top. */
fun yAt(spans: List<Span>, minute: Float): Float {
    val s = spans.at { minute >= it.start + it.lasts }
    return s.top + spans.into(s, minute - s.start) / maxOf(s.lasts, 1) * s.height
}

/** Where a drag of `by` minutes puts the row `s`, as top and bottom px: the whole block, or one edge. */
fun landing(spans: List<Span>, s: Span, edge: String?, by: Int): Pair<Float, Float> =
    when (edge) {
        "start" -> yAt(spans, (s.start + by).toFloat()) to s.top + s.height
        "end" -> s.top to yAt(spans, (s.start + s.lasts + by).toFloat())
        else -> yAt(spans, (s.start + by).toFloat()).let { it to it + s.height }
    }

/** Minutes as a drag reads them: the nearest fifteen. */
fun step(minutes: Float): Int = (minutes / 15f).roundToInt() * 15

/** The minutes a drag of row `s` to `y` reads, from the minute of the edge held: a gap after it costs nothing. */
fun draggedBy(spans: List<Span>, s: Span, edge: String?, y: Float): Int =
    step(minuteAt(spans, y) - if (edge == "end") s.start + s.lasts else s.start)

/** What a drop tells the engine: the hour, which edge for a resize, and by how many minutes. */
fun dragged(event: Value, by: Int, edge: String? = null): Value =
    Value.Fields(
        listOfNotNull(
            Field("block", event),
            edge?.let { Field("edge", Value.Text(it)) },
            Field("by", Value.Number(by.toDouble())),
        ),
    )

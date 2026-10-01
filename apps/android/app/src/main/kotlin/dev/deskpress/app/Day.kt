package dev.deskpress.app

import dev.deskpress.engine.Field
import dev.deskpress.engine.Value

/** Which entries of a Day are hours a drag moves among: the timed ones but the legs, as the engine counts them. */
fun hours(items: List<Value>): List<Int> =
    items.indices.filter { items[it].field("time").text().isNotEmpty() && items[it].field("leg") != Value.Bool(true) }

/**
 * Where the hour `from` lands after a drag of `offset`, given the centre of each hour: past every
 * centre it crosses, and never past a locked hour.
 */
fun landing(centres: List<Float>, locked: List<Boolean>, from: Int, offset: Float): Int {
    val at = centres[from] + offset
    var to = from
    if (offset > 0) {
        while (to + 1 < centres.size && !locked[to + 1] && centres[to + 1] < at) to++
    } else {
        while (to > 0 && !locked[to - 1] && centres[to - 1] > at) to--
    }
    return to
}

/** What a drop tells the engine: the hour moved and its new place among the hours. */
fun moved(event: Value, to: Int): Value = Value.Fields(listOf(Field("block", event), Field("to", Value.Number(to.toDouble()))))

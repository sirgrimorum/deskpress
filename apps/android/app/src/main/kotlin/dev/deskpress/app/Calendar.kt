package dev.deskpress.app

import android.content.ContentResolver
import android.content.ContentUris
import android.content.ContentValues
import android.provider.CalendarContract.Calendars
import android.provider.CalendarContract.Events
import android.provider.CalendarContract.Reminders
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import dev.deskpress.engine.Event
import dev.deskpress.engine.Plan
import java.time.Duration
import java.time.LocalDateTime
import java.time.ZoneId

/** The calendar row written for one event id, and the fingerprint it was written with. */
data class Link(val row: Long, val fingerprint: String)

/**
 * What this device wrote for one pack: the calendar the person picked, and a row per event id.
 * Only these rows are ever changed or deleted, so events the app did not create stay untouched.
 */
data class Ledger(val calendar: Long, val links: Map<String, Link> = emptyMap()) {
    fun known(): Map<String, String> = links.mapValues { it.value.fingerprint }

    /** One line for the calendar, then one per event, tab separated, in id order. */
    fun encode(): String = buildString {
        append("calendar\t$calendar\n")
        for ((id, link) in links.toSortedMap()) append("$id\t${link.row}\t${link.fingerprint}\n")
    }

    companion object {
        /** The ledger a file holds, or null when it holds none: a damaged file starts over. */
        fun decode(text: String): Ledger? {
            val lines = text.lines().filter(String::isNotEmpty).map { it.split("\t") }
            val calendar = lines.firstOrNull()?.takeIf { it.size == 2 && it[0] == "calendar" }?.get(1)?.toLongOrNull()
            val links = lines.drop(1).mapNotNull { l -> l.getOrNull(1)?.toLongOrNull()?.takeIf { l.size == 3 }?.let { l[0] to Link(it, l[2]) } }
            return calendar?.let { Ledger(it, links.toMap()) }
        }
    }
}

/** How many events a plan adds, changes and removes, as the person confirms it. */
fun summary(plan: Plan): String = "${plan.add.size} new, ${plan.change.size} changed, ${plan.remove.size} removed"

/** A pack-local `YYYY-MM-DDTHH:MM` as an instant, in milliseconds. */
fun millis(stamp: String, zone: ZoneId): Long = LocalDateTime.parse(stamp).atZone(zone).toInstant().toEpochMilli()

/** Minutes from the reminder to the start, or null for an event with none. */
fun minutes(event: Event): Long? =
    event.reminder.takeIf { it.isNotEmpty() }?.let { Duration.between(LocalDateTime.parse(it), LocalDateTime.parse(event.start)).toMinutes() }

/** The rows of one calendar, as `apply` needs them. A row outside `calendar` is never touched. */
interface Rows {
    fun insert(calendar: Long, event: Event): Long

    /** False when the row is gone, deleted by the person in their calendar app. */
    fun update(calendar: Long, row: Long, event: Event): Boolean

    fun delete(calendar: Long, row: Long)
}

/** The ledger after a sync, and why it stopped short, if it did. */
data class Applied(val ledger: Ledger, val failed: RuntimeException? = null)

/**
 * Writes `plan` into the ledger's calendar. What was written before a failure stays in the
 * ledger, so the next sync changes those rows instead of writing them twice.
 */
fun apply(plan: Plan, ledger: Ledger, rows: Rows): Applied {
    val links = ledger.links.toMutableMap()
    val failed =
        try {
            for (id in plan.remove) {
                links[id]?.let { rows.delete(ledger.calendar, it.row) }
                links.remove(id)
            }
            for (event in plan.add + plan.change) {
                val old = links[event.id]
                val row = if (old != null && rows.update(ledger.calendar, old.row, event)) old.row else rows.insert(ledger.calendar, event)
                links[event.id] = Link(row, event.fingerprint)
            }
            null
        } catch (e: RuntimeException) {
            e
        }
    return Applied(ledger.copy(links = links), failed)
}

/** The calendars this phone lets the app write to, as (id, name), the account after the name. */
fun calendars(resolver: ContentResolver): List<Pair<Long, String>> {
    val columns = arrayOf(Calendars._ID, Calendars.CALENDAR_DISPLAY_NAME, Calendars.ACCOUNT_NAME)
    val writable = "${Calendars.CALENDAR_ACCESS_LEVEL} >= ${Calendars.CAL_ACCESS_CONTRIBUTOR}"
    val found = mutableListOf<Pair<Long, String>>()
    resolver.query(Calendars.CONTENT_URI, columns, writable, null, null)?.use { c ->
        while (c.moveToNext()) found += c.getLong(0) to "${c.getString(1)} (${c.getString(2)})"
    }
    return found
}

/** The rows of the phone's calendar provider, times in each event's zones, else `zone`, the pack's. */
class ContentRows(private val resolver: ContentResolver, private val zone: ZoneId) : Rows {
    private fun values(event: Event) = ContentValues().apply {
        val (starts, ends) = zoneOr(event.zone, zone) to zoneOr(event.endZone, zone)
        put(Events.DTSTART, millis(event.start, starts))
        put(Events.DTEND, millis(event.end, ends))
        put(Events.EVENT_TIMEZONE, starts.id)
        put(Events.EVENT_END_TIMEZONE, ends.id)
        put(Events.TITLE, event.title)
        put(Events.EVENT_LOCATION, event.location)
        put(Events.DESCRIPTION, event.notes)
    }

    private fun remind(row: Long, event: Event) {
        resolver.delete(Reminders.CONTENT_URI, "${Reminders.EVENT_ID} = ?", arrayOf("$row"))
        val minutes = minutes(event) ?: return
        val values = ContentValues().apply {
            put(Reminders.EVENT_ID, row)
            put(Reminders.MINUTES, minutes)
            put(Reminders.METHOD, Reminders.METHOD_ALERT)
        }
        resolver.insert(Reminders.CONTENT_URI, values)
    }

    override fun insert(calendar: Long, event: Event): Long {
        val values = values(event).apply { put(Events.CALENDAR_ID, calendar) }
        val row = resolver.insert(Events.CONTENT_URI, values)?.let(ContentUris::parseId) ?: error("the calendar refused ${event.id}")
        remind(row, event)
        return row
    }

    private fun mine(calendar: Long) = "${Events.CALENDAR_ID} = $calendar"

    override fun update(calendar: Long, row: Long, event: Event): Boolean {
        val uri = ContentUris.withAppendedId(Events.CONTENT_URI, row)
        val alive = "${mine(calendar)} AND ${Events.DELETED} = 0"
        if (resolver.update(uri, values(event), alive, null) == 0) return false
        remind(row, event)
        return true
    }

    override fun delete(calendar: Long, row: Long) {
        resolver.delete(ContentUris.withAppendedId(Events.CONTENT_URI, row), mine(calendar), null)
    }
}

/** Which calendar the pack's events go to, asked once per pack. */
@Composable
fun PickCalendar(calendars: List<Pair<Long, String>>, pick: (Long) -> Unit, dismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = dismiss,
        title = { Text("Which calendar?") },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                for ((id, name) in calendars) TextButton({ pick(id) }) { Text(name) }
            }
        },
        confirmButton = {},
        dismissButton = { TextButton(dismiss) { Text("Cancel") } },
    )
}

/** What a sync would do, to confirm before anything is written. */
@Composable
fun ConfirmSync(plan: Plan, apply: () -> Unit, dismiss: () -> Unit) {
    fun line(mark: Char, e: Event) = "$mark ${e.start.replace('T', ' ')} ${e.title}"
    val lines = plan.add.map { line('+', it) } + plan.change.map { line('~', it) }
    AlertDialog(
        onDismissRequest = dismiss,
        title = { Text("Update the calendar") },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                Text(summary(plan))
                for (line in lines) Text(line)
            }
        },
        confirmButton = { TextButton(apply) { Text("Apply") } },
        dismissButton = { TextButton(dismiss) { Text("Cancel") } },
    )
}

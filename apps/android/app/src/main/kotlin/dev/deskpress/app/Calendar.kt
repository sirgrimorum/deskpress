package dev.deskpress.app

import android.content.ContentProviderOperation
import android.content.ContentProviderResult
import android.content.ContentResolver
import android.content.ContentUris
import android.content.ContentValues
import android.content.OperationApplicationException
import android.os.RemoteException
import android.provider.CalendarContract
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

/** What a write settled: the row each event ended at, the rows it dropped, and why it stopped. */
data class Wrote(
    val rows: Map<String, Long> = emptyMap(),
    val removed: List<Long> = emptyList(),
    val failed: RuntimeException? = null,
)

/** The rows of one calendar, as `apply` needs them. A row outside `calendar` is never touched. */
interface Rows {
    /**
     * Everything in one go (decision 0028): the rows of `remove` gone, then each event written at
     * its row, or at a new one where it has none. A failure reports what it settled before it.
     */
    fun write(calendar: Long, remove: List<Long>, write: List<Pair<Event, Long?>>): Wrote
}

/** One thing a batch asks of the calendar, decided here and translated by the host. */
sealed interface Op {
    data class Remove(val row: Long) : Op

    /** The event at a new row, its reminder pointing back at it. */
    data class Add(val event: Event) : Op

    /** The event over the row it had, if that row is still there. */
    data class Change(val event: Event, val row: Long) : Op

    /** The reminders of a row that was already there: the old ones gone, the new one in their place. */
    data class Remind(val event: Event, val row: Long) : Op
}

/** What the provider answered for one operation: the row an insert wrote, else the rows it touched. */
data class Done(val row: Long? = null, val rows: Int = 0)

/** What the first batch left over: the rows it settled, the events to write again, and the ops for both. */
data class Next(val rows: Map<String, Long>, val again: List<Event>, val ops: List<Op>)

/** The first batch: the rows the plan drops, then every event at its row or at a new one. */
fun batch(remove: List<Long>, write: List<Pair<Event, Long?>>): List<Op> =
    remove.map(Op::Remove) + write.map { (event, row) -> if (row == null) Op.Add(event) else Op.Change(event, row) }

/**
 * What the first batch settled, `done` being its answer to each of `write`, the removes dropped: a
 * new row is kept, a row that survived gets its reminders, and a row the person deleted in their
 * own calendar app is written again. The second batch adds first, so `again` lines up with it.
 */
fun next(write: List<Pair<Event, Long?>>, done: List<Done>): Next {
    val rows = mutableMapOf<String, Long>()
    val again = mutableListOf<Event>()
    val remind = mutableListOf<Op>()
    for ((at, pair) in write.withIndex()) {
        val (event, row) = pair
        when {
            row == null -> rows[event.id] = checkNotNull(done[at].row)
            done[at].rows == 0 -> again += event
            else -> {
                rows[event.id] = row
                remind += Op.Remind(event, row)
            }
        }
    }
    return Next(rows, again, again.map(Op::Add) + remind)
}

/** The rows the second batch wrote, for the events it added again. */
fun wrote(again: List<Event>, done: List<Done>): Map<String, Long> =
    again.mapIndexed { at, event -> event.id to checkNotNull(done[at].row) }.toMap()

/** The ledger after a sync, and why it did not happen, if it did not. */
data class Applied(val ledger: Ledger, val failed: RuntimeException? = null)

/**
 * Writes `plan` into the ledger's calendar in one batch. The ledger keeps exactly what the write
 * settled, so a sync that stops half way never writes the same event twice on the next one.
 */
fun apply(plan: Plan, ledger: Ledger, rows: Rows): Applied {
    val links = ledger.links.toMutableMap()
    val remove = plan.remove.mapNotNull { id -> links[id]?.row?.let { id to it } }
    val events = plan.add + plan.change
    val written = rows.write(ledger.calendar, remove.map { it.second }, events.map { it to links[it.id]?.row })
    val removed = written.removed.toSet()
    for ((id, row) in remove) if (row in removed) links.remove(id)
    for (event in events) written.rows[event.id]?.let { links[event.id] = Link(it, event.fingerprint) }
    return Applied(ledger.copy(links = links), written.failed)
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

    private fun uri(row: Long) = ContentUris.withAppendedId(Events.CONTENT_URI, row)

    private fun remind(minutes: Long) = ContentProviderOperation.newInsert(Reminders.CONTENT_URI)
        .withValue(Reminders.MINUTES, minutes)
        .withValue(Reminders.METHOD, Reminders.METHOD_ALERT)

    private fun gone(row: Long) = ContentProviderOperation.newDelete(Reminders.CONTENT_URI)
        .withSelection("${Reminders.EVENT_ID} = ?", arrayOf("$row")).build()

    /** One op as provider operations, its own first: a reminder of a new event points back at `at`. */
    private fun operations(calendar: Long, op: Op, at: Int): List<ContentProviderOperation> {
        val mine = "${Events.CALENDAR_ID} = $calendar"
        return when (op) {
            is Op.Remove ->
                listOf(ContentProviderOperation.newDelete(uri(op.row)).withSelection(mine, null).build())
            is Op.Add -> {
                val values = values(op.event).apply { put(Events.CALENDAR_ID, calendar) }
                val row = ContentProviderOperation.newInsert(Events.CONTENT_URI).withValues(values).build()
                val minutes = minutes(op.event)
                if (minutes == null) listOf(row)
                else listOf(row, remind(minutes).withValueBackReference(Reminders.EVENT_ID, at).build())
            }
            // A row the person deleted in their own calendar app is no longer alive, and no rows change.
            is Op.Change -> {
                val alive = "$mine AND ${Events.DELETED} = 0"
                listOf(ContentProviderOperation.newUpdate(uri(op.row)).withValues(values(op.event)).withSelection(alive, null).build())
            }
            is Op.Remind -> {
                val minutes = minutes(op.event)
                if (minutes == null) listOf(gone(op.row))
                else listOf(gone(op.row), remind(minutes).withValue(Reminders.EVENT_ID, op.row).build())
            }
        }
    }

    /** One trip to the calendar provider, with the answer to each op, in the order they were asked. */
    private fun run(calendar: Long, plan: List<Op>): List<Done> {
        val ops = mutableListOf<ContentProviderOperation>()
        val where = mutableListOf<Int>()
        for (op in plan) {
            where += ops.size
            ops += operations(calendar, op, ops.size)
        }
        val results: Array<ContentProviderResult> =
            try {
                resolver.applyBatch(CalendarContract.AUTHORITY, ArrayList(ops))
            } catch (e: OperationApplicationException) {
                throw IllegalStateException("the calendar refused the batch", e)
            } catch (e: RemoteException) {
                throw IllegalStateException("the calendar did not answer", e)
            }
        return where.map { Done(results[it].uri?.let(ContentUris::parseId), results[it].count ?: 0) }
    }

    /** The one place a calendar failure becomes data: what the batches settled, and what stopped them. */
    override fun write(calendar: Long, remove: List<Long>, write: List<Pair<Event, Long?>>): Wrote {
        var settled = Wrote()
        return try {
            val next = next(write, run(calendar, batch(remove, write)).drop(remove.size))
            settled = Wrote(next.rows, remove)
            if (next.ops.isEmpty()) settled
            else settled.copy(rows = next.rows + wrote(next.again, run(calendar, next.ops)))
        } catch (e: RuntimeException) {
            settled.copy(failed = e)
        }
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

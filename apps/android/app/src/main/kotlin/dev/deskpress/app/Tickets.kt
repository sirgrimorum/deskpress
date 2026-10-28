package dev.deskpress.app

import android.app.AlarmManager
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.core.app.NotificationCompat
import androidx.core.content.edit
import androidx.core.content.pm.ShortcutInfoCompat
import androidx.core.content.pm.ShortcutManagerCompat
import dev.deskpress.engine.Paper
import dev.deskpress.engine.Ticket
import java.net.URLDecoder
import java.net.URLEncoder
import java.time.ZoneId

/** A ticket as the phone keeps it: the instants it shows between, in epoch milliseconds, and its papers. */
data class Pass(val key: String, val from: Long, val until: Long, val title: String, val papers: List<Paper>)

/** The id of the launcher shortcut to the ticket showing now. */
const val TICKET = "ticket"

/** The engine's tickets not over at `now`, each on its own clocks, else the pack's. */
fun passes(tickets: List<Ticket>, timezone: ZoneId, now: Long): List<Pass> =
    tickets.mapNotNull { t ->
        runCatching {
            Pass(t.key, millis(t.from, zoneOr(t.fromZone, timezone)), millis(t.until, zoneOr(t.untilZone, timezone)), t.title, t.papers)
        }.getOrNull()?.takeIf { it.until > now }
    }

/** The ones showing at `now`. */
fun showing(passes: List<Pass>, now: Long): List<Pass> = passes.filter { now >= it.from && now < it.until }

/** The next instant one starts or stops showing; null when none will. */
fun turn(passes: List<Pass>, now: Long): Long? = passes.flatMap { listOf(it.from, it.until) }.filter { it > now }.minOrNull()

/** Passes as lines of tab-separated cells, each URL-encoded: key, from, until, title, then each paper's title and file. */
fun encodePasses(passes: List<Pass>): String =
    passes.joinToString("") { p ->
        val cells = listOf(p.key, p.from.toString(), p.until.toString(), p.title) + p.papers.flatMap { listOf(it.title, it.file) }
        cells.joinToString("\t") { URLEncoder.encode(it, "UTF-8") } + "\n"
    }

/** The passes kept by [encodePasses]; a line it cannot read is dropped. */
fun decodePasses(text: String): List<Pass> =
    text.lines().filter(String::isNotEmpty).mapNotNull { line ->
        val c = runCatching { line.split("\t").map { URLDecoder.decode(it, "UTF-8") } }.getOrNull()
        val from = c?.getOrNull(1)?.toLongOrNull()
        val until = c?.getOrNull(2)?.toLongOrNull()
        if (c == null || from == null || until == null) return@mapNotNull null
        val papers = c.drop(4).chunked(2).filter { it.size == 2 }.map { Paper(it[0], it[1]) }
        Pass(c[0], from, until, c.getOrElse(3) { "" }, papers)
    }

/**
 * Shows the open pack's tickets with the app closed (decision 0042): while one is due, a
 * notification the lock screen shows and a launcher shortcut, both opening its papers. One alarm,
 * at the next turn, keeps them current.
 */
class Ticketer(context: Context) {
    private val context = context.applicationContext
    private val prefs = context.getSharedPreferences("tickets", Context.MODE_PRIVATE)

    /** Keeps these, whose papers are in `folder`, the bundled pack's when null, and shows what is due. */
    fun set(passes: List<Pass>, folder: String?) {
        val text = encodePasses(passes)
        if (text != prefs.getString(PASSES, null) || folder != prefs.getString(FOLDER, null)) {
            prefs.edit {
                putString(PASSES, text)
                putString(FOLDER, folder)
            }
        }
        show()
    }

    /** The kept one by `key` while it shows, and the folder its papers are in; nothing else is handed out. */
    fun due(key: String): Pair<Pass, String?>? =
        showing(kept(), System.currentTimeMillis()).find { it.key == key }?.let { it to prefs.getString(FOLDER, null) }

    /** Shows what is due now, takes down what is not, and wakes at the next turn. */
    fun show() {
        val now = System.currentTimeMillis()
        val all = kept()
        val on = showing(all, now)
        val manager = context.getSystemService(NotificationManager::class.java)
        val tags = on.mapTo(HashSet()) { TAG + it.key }
        for (n in manager.activeNotifications) {
            if (n.tag?.startsWith(TAG) == true && n.tag !in tags) manager.cancel(n.tag, 0)
        }
        if (on.isNotEmpty() && context.canNotify()) {
            // Quiet: it is there to be found on the lock screen, not to ring.
            val channel = NotificationChannel(CHANNEL, "Tickets", NotificationManager.IMPORTANCE_DEFAULT).apply {
                setSound(null, null)
                enableVibration(false)
                lockscreenVisibility = Notification.VISIBILITY_PUBLIC
            }
            manager.createNotificationChannel(channel)
            for (p in on) manager.notify(TAG + p.key, 0, note(p))
        }
        shortcut(on.firstOrNull())
        val alarms = context.getSystemService(AlarmManager::class.java)
        val wake = PendingIntent.getBroadcast(context, 0, Intent(context, TicketReceiver::class.java), PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        val next = turn(all, now)
        when {
            next == null -> alarms.cancel(wake)
            Build.VERSION.SDK_INT < 31 || alarms.canScheduleExactAlarms() -> alarms.setExactAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, next, wake)
            else -> alarms.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, next, wake)
        }
    }

    private fun kept() = decodePasses(prefs.getString(PASSES, "").orEmpty())

    private fun note(p: Pass): Notification {
        val builder = NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(android.R.drawable.ic_menu_agenda)
            .setContentTitle(p.title)
            .setContentText(p.papers.joinToString(" · ") { it.title })
            .setCategory(NotificationCompat.CATEGORY_EVENT)
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setShowWhen(false)
            .setContentIntent(open(p.key, if (p.papers.size == 1) 0 else -1))
        // Each paper its own button where there is room; the rest are a tap on the notification away.
        if (p.papers.size > 1) p.papers.take(3).forEachIndexed { i, paper -> builder.addAction(0, paper.title, open(p.key, i)) }
        return builder.build()
    }

    /** The shortcut to the ticket showing, changed only when another one is. */
    private fun shortcut(p: Pass?) {
        val there = ShortcutManagerCompat.getDynamicShortcuts(context).find { it.id == TICKET }
        runCatching {
            when {
                p == null -> if (there != null) ShortcutManagerCompat.removeDynamicShortcuts(context, listOf(TICKET))
                there?.intent?.getStringExtra(KEY) != p.key || there.longLabel != p.title -> {
                    val intent = paper(p.key, if (p.papers.size == 1) 0 else -1).setAction(Intent.ACTION_VIEW)
                    val info = ShortcutInfoCompat.Builder(context, TICKET).setShortLabel(p.title).setLongLabel(p.title).setRank(0).setIntent(intent).build()
                    ShortcutManagerCompat.pushDynamicShortcut(context, info)
                }
            }
        }
    }

    // The data names the ticket and the paper, so each tap is its own intent.
    private fun paper(key: String, i: Int) =
        Intent(context, TicketActivity::class.java).setData(Uri.parse("ticket:${Uri.encode(key)}/$i")).putExtra(KEY, key).putExtra(PAPER, i)

    private fun open(key: String, i: Int) =
        PendingIntent.getActivity(context, 0, paper(key, i), PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)

    companion object {
        private const val PASSES = "passes"
        private const val FOLDER = "folder"
        private const val CHANNEL = "tickets"
        private const val TAG = "ticket:"
        const val KEY = "key"
        const val PAPER = "paper"
    }
}

/** Shows or takes down the tickets at each turn, and sets them again after a restart. */
class TicketReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) = Ticketer(context).show()
}

/**
 * A ticket's papers, over the lock screen: only a ticket kept as due now, read from the pack it
 * came from, and nothing else of the pack. Leaving it closes it (decision 0042).
 */
class TicketActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val (pass, folder) = intent.getStringExtra(Ticketer.KEY)?.let(Ticketer(this)::due) ?: return finish()
        val asked = intent.getIntExtra(Ticketer.PAPER, -1)
        setContent {
            var shown by remember { mutableIntStateOf(asked) }
            val tokens = BUILT_IN
            val paper = pass.papers.getOrNull(shown)
            if (paper != null) {
                // Back goes to the list only when the list is where it came from.
                Document(tokens, paper.title, { packFile(folder, paper.file) }) { if (asked >= 0) finish() else shown = -1 }
            } else {
                Scaffold(topBar = { Header(pass.title.uppercase(), ::finish) }, containerColor = tokens.color("paper")) { padding ->
                    LazyColumn(Modifier.padding(padding)) {
                        itemsIndexed(pass.papers) { i, p ->
                            Text(p.title, Modifier.fillMaxWidth().clickable { shown = i }.padding(tokens.size("spacing.margin")), style = style("body"))
                        }
                    }
                }
            }
        }
    }
}

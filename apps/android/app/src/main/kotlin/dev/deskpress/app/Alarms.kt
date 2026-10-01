package dev.deskpress.app

import android.Manifest
import android.app.AlarmManager
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.media.AudioAttributes
import android.media.RingtoneManager
import android.net.Uri
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import androidx.core.content.edit
import dev.deskpress.engine.Alarm
import java.net.URLDecoder
import java.net.URLEncoder
import java.time.ZoneId

/** An alarm as the phone sets it: its key, the instant in epoch milliseconds, and what it says. */
data class Ring(val key: String, val at: Long, val title: String, val text: String)

/** The most alarms armed at once, soonest first: the phone allows 500 an app, and each ring arms the next. */
const val MOST_RINGS = 100

/** The engine's alarms still to come after `now`, each on its own clock, else the pack's; soonest first. */
fun rings(alarms: List<Alarm>, timezone: ZoneId, now: Long): List<Ring> =
    alarms.mapNotNull { a ->
        val at = runCatching { millis(a.at, zoneOr(a.zone, timezone)) }.getOrNull()
        at?.takeIf { it > now }?.let { Ring(a.key, it, a.title, a.text) }
    }.sortedBy { it.at }

/** What moving from `before` to `rings` asks of the phone: the ones to cancel, and the soonest not armed yet. */
fun slide(before: List<Ring>, armed: Set<Ring>, rings: List<Ring>): Pair<List<Ring>, List<Ring>> {
    val window = rings.take(MOST_RINGS)
    val keys = window.mapTo(HashSet()) { it.key }
    return before.take(MOST_RINGS).filter { it.key !in keys } to window.filter { it !in armed }
}

/** Rings as lines of tab-separated cells, each cell URL-encoded. */
fun encodeRings(rings: List<Ring>): String =
    rings.joinToString("") { r -> listOf(r.key, r.at.toString(), r.title, r.text).joinToString("\t") { URLEncoder.encode(it, "UTF-8") } + "\n" }

/** The rings kept by [encodeRings]; a line it cannot read is dropped. */
fun decodeRings(text: String): List<Ring> =
    text.lines().filter(String::isNotEmpty).mapNotNull { line ->
        val c = runCatching { line.split("\t").map { URLDecoder.decode(it, "UTF-8") } }.getOrNull()
        c?.getOrNull(1)?.toLongOrNull()?.let { Ring(c[0], it, c.getOrElse(2) { "" }, c.getOrElse(3) { "" }) }
    }

/** Whether the phone lets this app show a notification, so an alarm can ring. */
fun Context.canNotify() =
    Build.VERSION.SDK_INT < 33 || ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED

/**
 * Sets the open pack's alarms with the phone's alarm service, so they ring with the app closed,
 * and keeps them all to arm the next as one rings, and again after a restart.
 */
class Ringer(context: Context) {
    private val context = context.applicationContext
    private val prefs = context.getSharedPreferences("alarms", Context.MODE_PRIVATE)
    private val manager = context.getSystemService(AlarmManager::class.java)
    // What this one set already, so a keep that changes no alarm costs no call to the phone.
    private var armed = emptySet<Ring>()

    fun set(rings: List<Ring>) {
        val before = kept()
        val (cancel, arm) = slide(before, armed, rings)
        for (old in cancel) manager.cancel(intent(old))
        // Exact where the phone allows it; else the system may run it a few minutes late.
        val exact = Build.VERSION.SDK_INT < 31 || manager.canScheduleExactAlarms()
        for (r in arm) {
            if (exact) {
                manager.setExactAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, r.at, intent(r))
            } else {
                manager.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, r.at, intent(r))
            }
        }
        armed = rings.take(MOST_RINGS).toSet()
        if (rings != before) prefs.edit { putString(RINGS, encodeRings(rings)) }
    }

    /** The kept ones still to come: after a ring the phone holds the rest, after a restart none. */
    fun again(restarted: Boolean) {
        val all = kept()
        if (!restarted) armed = all.take(MOST_RINGS).toSet()
        set(all.filter { it.at > System.currentTimeMillis() })
    }

    private fun kept() = decodeRings(prefs.getString(RINGS, "").orEmpty())

    private fun intent(r: Ring): PendingIntent {
        // The data names the alarm, so each key is its own and cancelling one leaves the rest.
        val ring = Intent(context, AlarmReceiver::class.java)
            .setData(Uri.parse("alarm:${Uri.encode(r.key)}"))
            .putExtra(TITLE, r.title)
            .putExtra(TEXT, r.text)
        return PendingIntent.getBroadcast(context, 0, ring, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
    }

    companion object {
        private const val RINGS = "rings"
        const val TITLE = "title"
        const val TEXT = "text"
    }
}

/** Rings an alarm as a notification with the alarm sound and arms the next; after a restart, sets them again. */
class AlarmReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action == Intent.ACTION_BOOT_COMPLETED || intent.action == Intent.ACTION_MY_PACKAGE_REPLACED) {
            Ringer(context).again(restarted = true)
            return
        }
        Ringer(context).again(restarted = false)
        if (!context.canNotify()) return
        val sound = RingtoneManager.getDefaultUri(RingtoneManager.TYPE_ALARM)
        val channel = NotificationChannel(CHANNEL, "Alarms", NotificationManager.IMPORTANCE_HIGH).apply {
            setSound(sound, AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_ALARM).build())
            enableVibration(true)
        }
        context.getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
        val open = PendingIntent.getActivity(
            context,
            0,
            Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_IMMUTABLE,
        )
        val note = NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(android.R.drawable.ic_lock_idle_alarm)
            .setContentTitle(intent.getStringExtra(Ringer.TITLE))
            .setContentText(intent.getStringExtra(Ringer.TEXT))
            .setCategory(NotificationCompat.CATEGORY_ALARM)
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setContentIntent(open)
            .setAutoCancel(true)
            .build()
        NotificationManagerCompat.from(context).notify(intent.dataString.hashCode(), note)
    }

    companion object {
        private const val CHANNEL = "alarms"
    }
}

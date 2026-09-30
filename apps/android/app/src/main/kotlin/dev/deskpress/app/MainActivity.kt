package dev.deskpress.app

import android.Manifest.permission.ACCESS_COARSE_LOCATION
import android.Manifest.permission.ACCESS_FINE_LOCATION
import android.Manifest.permission.READ_CALENDAR
import android.Manifest.permission.WRITE_CALENDAR
import android.annotation.SuppressLint
import android.app.ActivityManager
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager.PERMISSION_GRANTED
import android.location.LocationListener
import android.location.LocationManager
import android.media.RingtoneManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Looper
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager
import android.widget.Toast
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts.CreateDocument
import androidx.activity.result.contract.ActivityResultContracts.OpenDocumentTree
import androidx.activity.result.contract.ActivityResultContracts.RequestMultiplePermissions
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_WEAK
import androidx.biometric.BiometricManager.Authenticators.DEVICE_CREDENTIAL
import androidx.biometric.BiometricPrompt
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.core.content.ContextCompat
import androidx.core.content.pm.ShortcutInfoCompat
import androidx.core.content.pm.ShortcutManagerCompat
import androidx.core.content.edit
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.compose.LifecycleStartEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import dev.deskpress.engine.CallException
import dev.deskpress.engine.Edit
import dev.deskpress.engine.Plan
import dev.deskpress.engine.Shortcut
import java.io.File
import java.io.IOException
import java.time.LocalDateTime
import java.time.ZoneId
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** The pack a fresh install opens: a folder of the repo's examples, shipped as assets. */
private const val PACK = "one-day"

/** The folder the person picked, kept across runs with its read and write permission. */
private const val FOLDER = "folder"
private const val SCALE = "scale"
private const val NAVIGATOR = "navigator"

/** The id of a question a launcher or the phone's assistant opened the app at (decision 0027). */
private const val QUESTION = "dev.deskpress.question"

private val LOCATION = arrayOf(ACCESS_FINE_LOCATION, ACCESS_COARSE_LOCATION)

private val CALENDAR = arrayOf(READ_CALENDAR, WRITE_CALENDAR)

/** A calendar sync on its way: the calendar to pick, or the plan to confirm. */
private sealed interface Syncing {
    data class Pick(val scope: String, val calendars: List<Pair<Long, String>>) : Syncing

    data class Confirm(val plan: Plan, val ledger: Ledger) : Syncing
}

/** A question a data sync waits on: whether to fetch from these hosts, or a secret. */
private sealed interface Asking {
    val answer: CompletableDeferred<*>

    data class Hosts(val hosts: List<String>, override val answer: CompletableDeferred<Boolean>) : Asking

    data class Secret(val name: String, val host: String, override val answer: CompletableDeferred<String?>) : Asking
}

class MainActivity : FragmentActivity() {
    private val prefs by lazy { getSharedPreferences("shell", MODE_PRIVATE) }

    // The question a shortcut asked for, until the screen has opened at it.
    private val asked = MutableStateFlow<String?>(null)

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        asked.value = intent.getStringExtra(QUESTION)
    }

    /**
     * Offers these questions to the launcher and the phone's assistant. The words only: what the
     * pack answers is drawn in the app and never handed over (decision 0027).
     */
    private fun offer(questions: List<Shortcut>) {
        val room = ShortcutManagerCompat.getMaxShortcutCountPerActivity(this)
        val shortcuts = questions.take(room).map { q ->
            val open = Intent(this, MainActivity::class.java)
                .setAction(Intent.ACTION_VIEW)
                .putExtra(QUESTION, q.id)
            ShortcutInfoCompat.Builder(this, "ask.${q.id}")
                .setShortLabel(q.ask)
                .setLongLabel(q.ask)
                .setIntent(open)
                .build()
        }
        runCatching { ShortcutManagerCompat.setDynamicShortcuts(this, shortcuts) }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        asked.value = intent.getStringExtra(QUESTION)
        // The e2e flows freeze the clock at a pack-local moment, the extra "now".
        val frozen = intent.getStringExtra("now")?.let(LocalDateTime::parse)
        // And the device's position, the extra "at" as "lat,lon": under them nothing is tracked.
        val pinned = intent.getStringExtra("at")?.split(",")?.mapNotNull(String::toDoubleOrNull)
        setContent {
            var folder by remember { mutableStateOf(prefs.getString(FOLDER, null)) }
            // The shell screen over the pack: "check", "design", "settings", or none.
            var page by rememberSaveable { mutableStateOf<String?>(null) }
            var shell by remember {
                mutableStateOf(Shell(prefs.getFloat(SCALE, 1f), prefs.getString(NAVIGATOR, "").orEmpty()))
            }
            val picker = rememberLauncherForActivityResult(OpenDocumentTree()) { uri ->
                if (uri != null) {
                    // Write access when the folder gives it, else read only: edits then offer a copy.
                    val read = Intent.FLAG_GRANT_READ_URI_PERMISSION
                    try {
                        contentResolver.takePersistableUriPermission(uri, read or Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
                    } catch (_: SecurityException) {
                        contentResolver.takePersistableUriPermission(uri, read)
                    }
                    // Only the open folder is kept: the phone allows a limited number of grants.
                    for (old in contentResolver.persistedUriPermissions) {
                        val held = (if (old.isReadPermission) read else 0) or (if (old.isWritePermission) Intent.FLAG_GRANT_WRITE_URI_PERMISSION else 0)
                        if (old.uri != uri) contentResolver.releasePersistableUriPermission(old.uri, held)
                    }
                    prefs.edit { putString(FOLDER, uri.toString()) }
                    // The pack before stops, its timers and fetches with it, before the new one loads.
                    viewModelStore.clear()
                    folder = uri.toString()
                }
            }
            val model = viewModel(key = folder ?: PACK) {
                val now = { zone: ZoneId -> frozen ?: LocalDateTime.now(zone) }
                val facts = Facts(File(filesDir, "facts"))
                val picked = folder?.let(Uri::parse)
                if (picked != null) {
                    PackViewModel({ read(picked) }, now, facts = facts, write = { f, t -> write(picked, f, t) })
                } else {
                    val overlay = Overlay(File(filesDir, "packs/$PACK"))
                    PackViewModel({ overlay.over(assets.pack(PACK)) }, now, facts = facts, write = overlay::write)
                }
            }

            val scope = rememberCoroutineScope()
            // A theme file the folder refused, written where the person picks (decision 0016).
            var unwritten by remember { mutableStateOf<Edit?>(null) }
            val copier = rememberLauncherForActivityResult(CreateDocument("application/yaml")) { uri ->
                val copy = unwritten
                unwritten = null
                if (uri != null && copy != null) {
                    scope.launch {
                        val saved = withContext(Dispatchers.IO) {
                            runCatching { (contentResolver.openOutputStream(uri, "wt") ?: throw IOException("the file could not be opened")).bufferedWriter().use { it.write(copy.text) } }
                        }
                        say(if (saved.isSuccess) "A copy of ${copy.file} is saved." else "The copy was not saved: ${saved.exceptionOrNull()?.message}")
                    }
                }
            }

            // The pack file shown full screen, as (file, title), over the pack's screen.
            var doc by remember { mutableStateOf<Pair<String, String>?>(null) }

            var located by remember { mutableStateOf(canLocate()) }
            val ask = rememberLauncherForActivityResult(RequestMultiplePermissions()) {
                located = canLocate()
                if (!located) say("Without the position, the app cannot tell where you are.")
            }

            // The calendar: which one each pack writes to and what it wrote, one file per pack.
            val ledgers = remember { Facts(File(filesDir, "calendar"), "tsv") }
            var syncing by remember { mutableStateOf<Syncing?>(null) }
            var waiting by remember { mutableStateOf<String?>(null) }
            val confirm: (String, Ledger) -> Unit = { at, ledger ->
                scope.launch {
                    val plan =
                        try {
                            model.plan(at, ledger.known())
                        } catch (e: CallException.Refused) {
                            say(e.detail)
                            null
                        } ?: return@launch
                    if (plan.add.isEmpty() && plan.change.isEmpty() && plan.remove.isEmpty()) {
                        say("The calendar is up to date.")
                    } else {
                        syncing = Syncing.Confirm(plan, ledger)
                    }
                }
            }
            val begin: (String) -> Unit = { at ->
                scope.launch {
                    // A ledger whose calendar is gone, deleted or its account removed, is picked again.
                    val (ledger, found) = withContext(Dispatchers.IO) {
                        val found = calendars(contentResolver)
                        ledgers.read(model.packId)?.let(Ledger::decode)?.takeIf { l -> found.any { it.first == l.calendar } } to found
                    }
                    when {
                        ledger != null -> confirm(at, ledger)
                        found.isEmpty() -> say("No calendar on this phone to write to.")
                        else -> syncing = Syncing.Pick(at, found)
                    }
                }
            }
            val askCalendar = rememberLauncherForActivityResult(RequestMultiplePermissions()) {
                val at = waiting
                waiting = null
                if (at == null) return@rememberLauncherForActivityResult
                if (canCalendar()) begin(at) else say("Without the calendar permission, nothing is written.")
            }
            val write: (Plan, Ledger) -> Unit = write@{ plan, ledger ->
                // A second tap on Apply before the dialog goes writes nothing twice.
                if (syncing == null) return@write
                syncing = null
                scope.launch {
                    val done = withContext(Dispatchers.IO) {
                        apply(plan, ledger, ContentRows(contentResolver, model.timezone)).also { ledgers.write(model.packId, it.ledger.encode()) }
                    }
                    say(if (done.failed == null) "The calendar is updated: ${summary(plan)}." else "The calendar stopped short: ${done.failed.message}")
                }
            }

            // A data sync: the hosts each pack may fetch from, its sealed secrets, and the question
            // on screen. One left open when the screen goes is dropped, and the fetch with it.
            val approved = remember { Facts(File(filesDir, "hosts"), "txt") }
            val secrets = remember { Secrets(File(filesDir, "secrets")) }
            var asking by remember { mutableStateOf<Asking?>(null) }
            DisposableEffect(Unit) { onDispose { asking?.answer?.cancel() } }

            LaunchedEffect(model) {
                model.allow = { hosts ->
                    val list = hosts.joinToString("\n")
                    withContext(Dispatchers.IO) { approved.read(model.packId) } == list || run {
                        val answer = CompletableDeferred<Boolean>()
                        asking = Asking.Hosts(hosts, answer)
                        answer.await().also { if (it) withContext(Dispatchers.IO) { approved.write(model.packId, list) } }
                    }
                }
                model.secret = { name, host ->
                    val slot = "${model.packId}/$host/$name"
                    withContext(Dispatchers.IO) { secrets.read(slot) } ?: run {
                        val answer = CompletableDeferred<String?>()
                        asking = Asking.Secret(name, host, answer)
                        answer.await()?.also { withContext(Dispatchers.IO) { secrets.write(slot, it) } }
                    }
                }
                model.forget = { name, host -> scope.launch(Dispatchers.IO) { secrets.forget("${model.packId}/$host/$name") } }
                model.unlock = this@MainActivity::promptUnlock
                model.call = this@MainActivity::dial
                model.show = { file, title -> doc = file to title }
                model.map = this@MainActivity::map
                model.sync = { at ->
                    if (canCalendar()) begin(at)
                    else {
                        waiting = at
                        askCalendar.launch(CALENDAR)
                    }
                }
                model.unlocated = {
                    if (!located) ask.launch(LOCATION)
                    else say("No position yet. Try again in the open.")
                }
            }

            // The phone's own model where there is one (decision 0027), else no assistant at all.
            DisposableEffect(model) {
                var own: Assistant? = null
                val setup = scope.launch {
                    val found = Assistant.on() ?: return@launch
                    own = found
                    model.assistant = found::ask
                }
                onDispose {
                    setup.cancel()
                    model.assistant = null
                    own?.close()
                }
            }

            val state by model.state.collectAsStateWithLifecycle()

            // A pack with regions asks once for the position, and follows it only while started.
            val fenced = (state as? PackState.Showing)?.view?.watch?.regions?.isNotEmpty() == true
            LaunchedEffect(fenced) { if (fenced && !located && frozen == null) ask.launch(LOCATION) }
            LifecycleStartEffect(located, model, state is PackState.Showing, fenced) {
                if (frozen != null && pinned?.size == 2) model.moved(pinned[0], pinned[1])
                // Checked again at each start: the permission may have been granted in the settings.
                val stop = if (canLocate() && frozen == null) track(model, fenced) else null
                onStopOrDispose { stop?.invoke() }
            }

            val shown = state as? PackState.Showing
            val tree = shown?.view?.tree
            val kid = tree?.kid == true
            val alarm = tree?.nodes?.firstOrNull { it.kind == "Screen" }?.props?.get("alarm")?.text() == "true"

            // A child holds the phone: pin the app so it cannot be left. Not under a frozen
            // clock: the e2e flows would meet the system's pin dialog, which outlives the app.
            LaunchedEffect(kid) {
                if (frozen != null) return@LaunchedEffect
                val am = getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
                val pinned = am.lockTaskModeState != ActivityManager.LOCK_TASK_MODE_NONE
                try {
                    if (kid && !pinned) startLockTask() else if (!kid && pinned) stopLockTask()
                } catch (_: IllegalStateException) {
                    // Where the device does not allow pinning, the app simply stays unpinned.
                }
            }

            // Runs when the alarm screen comes up, not on every draw of it.
            LaunchedEffect(alarm) { if (alarm) soundAlarm() }

            // The questions the pack offers outside the app, and the one a shortcut opened it at.
            val offers = shown?.view?.shortcuts.orEmpty()
            LaunchedEffect(offers) { offer(offers) }
            val question by asked.collectAsStateWithLifecycle()
            LaunchedEffect(question, model) {
                question?.let {
                    model.ask(it)
                    asked.value = null
                }
            }

            val dark = isSystemInDarkTheme()
            // A picture a pack carries, for the maps it draws. Unreadable is no picture.
            val packFile = remember(folder) {
                { path: String -> runCatching { file(folder, path) }.getOrNull() }
            }
            CompositionLocalProvider(LocalShell provides shell, LocalPackFile provides packFile) {
                MaterialTheme(if (dark) darkColorScheme() else lightColorScheme()) {
                    val close = { page = null }
                    if (page != null) BackHandler(onBack = close)
                    val open = doc
                    val menu =
                        remember(picker) {
                            Menu({ picker.launch(null) }, { page = "check" }, { page = "design" }, { page = "settings" })
                        }
                    when (val s = syncing) {
                        is Syncing.Pick -> PickCalendar(s.calendars, { id ->
                            syncing = null
                            confirm(s.scope, Ledger(id))
                        }) { syncing = null }
                        is Syncing.Confirm -> ConfirmSync(s.plan, { write(s.plan, s.ledger) }) { syncing = null }
                        null -> {}
                    }
                    when (val a = asking) {
                        is Asking.Hosts -> AllowHosts(a.hosts) { asking = null; a.answer.complete(it) }
                        is Asking.Secret -> AskSecret(a.name, a.host) { asking = null; a.answer.complete(it) }
                        null -> {}
                    }
                    when {
                        open != null -> Document(state, open.second, { file(folder, open.first) }) { doc = null }
                        page == "check" -> Check(state, close)
                        page == "design" -> Design(state, model::edit, { copy -> unwritten = copy; copier.launch(copy.file.substringAfterLast('/')) }, close)
                        page == "settings" ->
                            Settings(shell, { new ->
                                shell = new
                                prefs.edit { putFloat(SCALE, new.scale); putString(NAVIGATOR, new.navigator) }
                            }, close)
                        else -> PackScreen(state, model::act, menu)
                    }
                }
            }
        }
    }

    /** Starts `intent`, and says whether the phone had anything to take it. */
    private fun open(intent: Intent): Boolean =
        try {
            startActivity(intent)
            true
        } catch (_: ActivityNotFoundException) {
            false
        }

    /** The dialer with `number` typed in: the person presses call. A tablet has no dialer. */
    private fun dial(number: String) {
        open(Intent(Intent.ACTION_DIAL, Uri.fromParts("tel", number, null)))
    }

    /**
     * A map app at `lat`, `lon` with a pin named `label`: the one the settings name, else whichever
     * the phone offers. A named app that is not installed falls back to the phone's own choice.
     */
    private fun map(lat: Double, lon: Double, label: String) {
        val go = { Intent(Intent.ACTION_VIEW, Uri.parse("geo:$lat,$lon?q=$lat,$lon(${Uri.encode(label)})")) }
        val wanted = prefs.getString(NAVIGATOR, "").orEmpty()
        if (wanted.isEmpty() || !open(go().setPackage(wanted))) open(go())
    }

    private fun say(text: String) = Toast.makeText(this, text, Toast.LENGTH_LONG).show()

    private fun canCalendar() = CALENDAR.all { ContextCompat.checkSelfPermission(this, it) == PERMISSION_GRANTED }

    private fun canLocate() = LOCATION.any { ContextCompat.checkSelfPermission(this, it) == PERMISSION_GRANTED }

    /**
     * Hands `model` the last known position, then every new one, until the returned stop. Called
     * only with a location permission, which may still be taken back meanwhile. Every few seconds
     * when the pack has regions to arrive in, else once a minute, enough for the car's spot.
     */
    @SuppressLint("MissingPermission")
    private fun track(model: PackViewModel, fenced: Boolean): () -> Unit {
        val manager = getSystemService(LocationManager::class.java)
        val listener = LocationListener { model.moved(it.latitude, it.longitude) }
        return try {
            val providers = listOf(LocationManager.GPS_PROVIDER, LocationManager.NETWORK_PROVIDER).filter(manager::isProviderEnabled)
            providers.mapNotNull(manager::getLastKnownLocation).maxByOrNull { it.time }?.let { model.moved(it.latitude, it.longitude) }
            val (every, meters) = if (fenced) 5_000L to 10f else 60_000L to 50f
            providers.forEach { manager.requestLocationUpdates(it, every, meters, listener, Looper.getMainLooper()) }
            ({ manager.removeUpdates(listener) })
        } catch (_: SecurityException) {
            {}
        }
    }

    /** The bytes of `path` in the pack: the picked folder's, or the bundled pack's. */
    private fun file(folder: String?, path: String): ByteArray =
        folder?.let { bytes(Uri.parse(it), path) } ?: assets.open("$PACK/$path").use { it.readBytes() }

    /**
     * Asks for the phone's own lock: a fingerprint, a face, or the PIN or pattern. A phone with no
     * lock at all has nothing to ask for, so it lets through; any other reason the prompt cannot
     * show refuses.
     */
    private fun promptUnlock(done: (Boolean) -> Unit) {
        val manager = BiometricManager.from(this)
        val authenticators = BIOMETRIC_WEAK or DEVICE_CREDENTIAL
        when (manager.canAuthenticate(authenticators)) {
            BiometricManager.BIOMETRIC_SUCCESS -> {}
            BiometricManager.BIOMETRIC_ERROR_NONE_ENROLLED -> return done(true)
            else -> {
                say("The phone cannot ask for its lock right now.")
                return done(false)
            }
        }

        val executor = ContextCompat.getMainExecutor(this)
        val prompt =
            BiometricPrompt(
                this,
                executor,
                object : BiometricPrompt.AuthenticationCallback() {
                    override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                        done(true)
                    }

                    override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                        done(false)
                    }
                },
            )

        val info =
            BiometricPrompt.PromptInfo.Builder()
                .setTitle("Unlock")
                .setAllowedAuthenticators(authenticators)
                .build()

        prompt.authenticate(info)
    }

    private fun soundAlarm() {
        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                val vm = getSystemService(Context.VIBRATOR_MANAGER_SERVICE) as? VibratorManager
                vm?.defaultVibrator?.vibrate(VibrationEffect.createOneShot(500, VibrationEffect.DEFAULT_AMPLITUDE))
            } else {
                @Suppress("DEPRECATION")
                val vibrator = getSystemService(Context.VIBRATOR_SERVICE) as? Vibrator
                @Suppress("DEPRECATION")
                vibrator?.vibrate(500)
            }
        } catch (_: Exception) {
            // The alarm still sounds without the buzz; a phone with no vibrator is no reason to stop.
        }

        try {
            val uri = RingtoneManager.getDefaultUri(RingtoneManager.TYPE_NOTIFICATION)
            val ringtone = RingtoneManager.getRingtone(applicationContext, uri)
            ringtone?.play()
        } catch (_: Exception) {
            // The buzz and the screen still carry the alarm when the sound cannot play.
        }
    }

    /** A folder whose permission was taken back reads as a pack that cannot be read. */
    private fun read(folder: Uri): Map<String, String> =
        try {
            pack(folder)
        } catch (e: SecurityException) {
            throw IOException("the folder can no longer be read", e)
        }
}

package dev.deskpress.app

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dev.deskpress.engine.Alarm
import dev.deskpress.engine.CallException
import dev.deskpress.engine.Edit
import dev.deskpress.engine.Field
import dev.deskpress.engine.Finding
import dev.deskpress.engine.LoadException
import dev.deskpress.engine.LoadedPack
import dev.deskpress.engine.Plan
import dev.deskpress.engine.Taken
import dev.deskpress.engine.Value
import dev.deskpress.engine.View
import dev.deskpress.engine.World
import dev.deskpress.engine.decodeFacts
import dev.deskpress.engine.editTheme
import dev.deskpress.engine.encodeFacts
import dev.deskpress.engine.load
import java.io.File
import java.io.IOException
import java.net.URI
import java.net.URLEncoder
import java.nio.file.Files
import java.nio.file.StandardCopyOption
import java.time.Duration
import java.time.LocalDateTime
import java.time.ZoneId
import kotlin.text.Charsets.UTF_8
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

/**
 * Holds the pack across configuration changes. The engine decides; this only asks it again when
 * something happens: a tap, or the time its last answer said to wait for.
 */
class PackViewModel(
    files: () -> Map<String, String>,
    private val now: (ZoneId) -> LocalDateTime = LocalDateTime::now,
    scope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate),
    private val io: CoroutineDispatcher = Dispatchers.IO,
    private val facts: Facts? = null,
    // Where an edited file goes: the pack's own folder, or the bundled pack's overlay.
    private val write: (String, String) -> Unit = { _, _ -> throw IOException("this pack is read only") },
    // A GET of an https address, run on `io`.
    private val fetch: (String) -> Reply = ::get,
) : ViewModel(scope) {
    private val _state = MutableStateFlow<PackState>(PackState.Loading)
    val state: StateFlow<PackState> = _state.asStateFlow()

    private var pack: LoadedPack? = null
    private var files = emptyMap<String, String>()
    /** The pack's timezone, the one its times are local to. */
    var timezone: ZoneId = ZoneId.of("UTC")
        private set
    // The plan's other zones the phone knows, by the name the pack gives them.
    private var zones = emptyMap<String, ZoneId>()
    /** The pack's id, which names the files the host keeps for it. */
    var packId = ""
        private set
    private var theme: Value = Value.Null
    private var warnings = emptyList<Finding>()
    // What actions kept, saved under the pack's id after each one, and when each was kept in
    // milliseconds, for a trip another phone sends (decision 0034).
    private val store = mutableMapOf<String, Value>()
    private val stamps = mutableMapOf<String, Long>()
    private var timer: Job? = null
    private var ringing: Job? = null
    // One write of the facts at a time, in the order they were asked for.
    private val disk = Mutex()
    private val _position = MutableStateFlow<Pair<Double, Double>?>(null)
    /** Where the device is, once it knows: the regions and the full-screen map follow it. */
    val position: StateFlow<Pair<Double, Double>?> = _position.asStateFlow()
    // The regions of the pack around the device.
    private var inside = emptyList<String>()
    /** The clock set by hand, in seconds on top of `now`. */
    var shift = 0L
    // The modules being fetched, and whether the person said no to fetching this session.
    private val syncing = mutableSetOf<String>()
    private var declined = false

    // The host's biometric or credential check, hooked by the activity.
    var unlock: (done: (Boolean) -> Unit) -> Unit = { it(false) }

    // The dialer with a number, and a pack file full screen: the activity's too.
    var call: (number: String) -> Unit = {}
    var show: (file: String, title: String) -> Unit = { _, _ -> }

    // A map at a point, the day's stops as directions, and what to do asked with no position.
    var map: (lat: Double, lon: Double, label: String) -> Unit = { _, _, _ -> }
    var route: (stops: List<Stop>) -> Unit = {}
    var unlocated: () -> Unit = {}

    // A calendar sync of a scope: the whole trip when empty, a date, or one event id.
    var sync: (scope: String) -> Unit = {}

    // The phone's own model, where it has one: a question and the only facts it may answer from,
    // and what it said. Null on a phone without one, and then `can.assistant` is false and the
    // pack shows no box to type in.
    var assistant: (suspend (question: String, facts: Value) -> String)? = null

    // Whether the pack may fetch from these hosts, a secret by its name for the host it goes to
    // (null when there is none), and forgetting one the server turned down: the activity asks the
    // person and keeps both. A secret goes only to the host it was given for.
    var allow: suspend (hosts: List<String>) -> Boolean = { false }
    var secret: suspend (name: String, host: String) -> String? = { _, _ -> null }
    var forget: (name: String, host: String) -> Unit = { _, _ -> }

    // The phone's alarm service, handed every alarm still to ring each time the pack or its facts change.
    var alarms: (List<Alarm>) -> Unit = {}
        set(value) {
            field = value
            ring()
        }

    init {
        viewModelScope.launch {
            _state.value = attempt { open(withContext(io) { files() }) }
            watch()
            ring()
        }
    }

    /** Loads `files` as the pack, in place of the one before, and draws its screen. */
    private suspend fun open(files: Map<String, String>): PackState {
        val loaded = withContext(io) { load("pack.yaml", files) }
        val tz = loaded.timezone()
        timezone = runCatching { ZoneId.of(tz) }.getOrNull() ?: return PackState.Failed("the timezone $tz is not one this phone knows")
        // The one before is not closed: a fetch or timer may still hold it, and the bindings'
        // cleaner frees it once nothing does.
        pack = loaded
        this.files = files
        zones = loaded.zones().mapNotNull { n -> runCatching { ZoneId.of(n) }.getOrNull()?.let { n to it } }.toMap()
        packId = loaded.id()
        theme = loaded.theme()
        warnings = loaded.warnings()
        withContext(io) {
            facts?.read(packId)?.let { store.putAll(decodeFacts(it)) }
            facts?.read("$packId.stamps")?.let { t ->
                for ((k, v) in decodeFacts(t)) if (v is Value.Number) stamps[k] = v.value.toLong()
            }
        }
        val world = here()
        return show(withContext(io) { loaded.screen(world) }, warnings, theme)
    }

    override fun onCleared() {
        pack?.close()
    }

    /**
     * Sets the value at `path` of the theme file, writes the file back and draws with it. `done`
     * gets why the edit was refused, or null when it was kept, and the edited file when it was
     * valid but could not be written, for the person to save a copy of.
     */
    fun edit(path: List<String>, value: String, done: (String?, Edit?) -> Unit) {
        viewModelScope.launch {
            // The edited file until it is written.
            var copy: Edit? = null
            val why =
                try {
                    val edited = withContext(io) { editTheme("pack.yaml", files, path, value) }
                    copy = edited
                    withContext(io) { write(edited.file, edited.text) }
                    _state.value = attempt { open(files + (edited.file to edited.text)) }
                    watch()
                    ring()
                    copy = null
                    null
                } catch (e: LoadException.Unreadable) {
                    e.detail
                } catch (e: LoadException.Invalid) {
                    e.errors.joinToString("\n") { it.message }
                } catch (e: IOException) {
                    "not written: ${e.message}"
                } catch (e: SecurityException) {
                    "not written: the folder can no longer be written. Open it again from the menu, or save a copy."
                }
            done(why, copy)
        }
    }

    /** Runs an action of the screen, as a tap on a node that names it. */
    fun act(action: String, arg: Value) {
        val pack = pack ?: return
        viewModelScope.launch {
            val world = here()
            val out =
                try {
                    withContext(io) { pack.dispatch(world, action, arg) }
                } catch (e: CallException.Refused) {
                    _state.value = PackState.Failed(e.detail)
                    return@launch
                }
            store.putAll(out.store)
            stamp(out.store.keys)
            if (out.store.isNotEmpty()) keep()
            _state.value = attempt { show(out.view, warnings, theme) }
            watch()
            if (out.store.isNotEmpty()) ring()
            for (cmd in out.commands) {
                when (cmd.name) {
                    "device.unlock" -> unlock { ok ->
                        val then = cmd.args["then"].text()
                        if (ok && then.isNotEmpty()) act(then, Value.Null)
                    }
                    "phone.call" -> call(cmd.args["number"].text())
                    "document.open" -> show(cmd.args["file"].text(), cmd.args["title"].text())
                    "location.get" -> {
                        val at = _position.value
                        val then = cmd.args["then"].text()
                        if (at == null) unlocated()
                        else if (then.isNotEmpty()) act(then, point(at.first, at.second))
                    }
                    "calendar.sync" -> sync(cmd.args["scope"].text())
                    // The orders are the host's; the pack's words go in as data (decision 0027).
                    "assistant.ask" -> {
                        val ask = assistant
                        val then = cmd.args["then"].text()
                        if (ask != null && then.isNotEmpty()) {
                            val said = ask(cmd.args["question"].text(), cmd.args["facts"] ?: Value.Null)
                            act(then, Value.Text(said))
                        }
                    }
                    "map.open" -> {
                        val lat = (cmd.args["lat"] as? Value.Number)?.value
                        val lon = (cmd.args["lon"] as? Value.Number)?.value
                        if (lat != null && lon != null) map(lat, lon, cmd.args["label"].text())
                    }
                    "map.route" -> {
                        val stops = (cmd.args["stops"] as? Value.Items)?.items.orEmpty().mapNotNull { s -> pin(s)?.let { Stop(it.lat, it.lon) } }
                        if (stops.isNotEmpty()) route(stops)
                    }
                    else -> if (cmd.name.endsWith(".sync")) refresh(cmd.name.removeSuffix(".sync"))
                }
            }
        }
    }

    /**
     * Opens the pack's questions at `id`, as a launcher shortcut asks for. A screen with no way
     * there is left as it is.
     */
    fun ask(id: String) {
        val pack = pack ?: return
        viewModelScope.launch {
            val world = here()
            val out =
                try {
                    withContext(io) {
                        pack.dispatch(world, "ask", Value.Null)
                        pack.dispatch(world, "show", Value.Text(id))
                    }
                } catch (e: CallException.Refused) {
                    return@launch
                }
            _state.value = attempt { show(out.view, warnings, theme) }
            watch()
        }
    }

    /**
     * The device is at `lat`, `lon`. The engine is asked again only when that changes the regions
     * it is inside, or on the first position, which makes `away` mean something.
     */
    fun moved(lat: Double, lon: Double) {
        val regions = (_state.value as? PackState.Showing)?.view?.watch?.regions.orEmpty()
        val now = inside(regions, lat, lon)
        val first = _position.value == null
        _position.value = lat to lon
        if (!first && now == inside) return
        inside = now
        redraw()
    }

    /** No position known any more: the pinned one was let go, and no real fix has come yet. */
    fun unpinned() {
        _position.value = null
        inside = emptyList()
        redraw()
    }

    /** What a calendar sync of `scope` would do, against the events `known` written before. */
    suspend fun plan(scope: String, known: Map<String, String>): Plan? {
        val pack = pack ?: return null
        val world = here()
        return withContext(io) { pack.calendar(world, scope, known) }
    }

    private fun here(local: LocalDateTime = clock()): World {
        val at = local.atZone(timezone)
        val others = zones.mapValues { at.withZoneSameInstant(it.value).toLocalDateTime() }
        val can = if (assistant == null) emptyList() else listOf("assistant")
        return world(local, store, inside, _position.value != null, others, can)
    }

    private fun update(call: suspend (LoadedPack) -> View) {
        val pack = pack ?: return
        viewModelScope.launch {
            _state.value = attempt { show(call(pack), warnings, theme) }
            watch()
        }
    }

    /**
     * Fetches what `module` syncs, or every automatic sync that is due when it is empty, and draws
     * with what came back. A module already being fetched is left to finish.
     */
    private fun refresh(module: String) {
        val pack = pack ?: return
        if (module.isEmpty() && declined) return
        viewModelScope.launch {
            // On the real clock: a stamp from the clock set by hand could lie in the future.
            val world = here(now(timezone))
            val requests =
                try {
                    withContext(io) { pack.requests(world, module) }.filter { it.module !in syncing }
                } catch (_: CallException.Refused) {
                    return@launch
                }
            if (requests.isEmpty()) return@launch
            // Taken before the question, so a second refresh while it is open neither asks nor fetches.
            val modules = requests.map { it.module }
            syncing += modules
            try {
                declined = !allow(withContext(io) { pack.hosts() })
                if (declined) return@launch
                for (r in requests) {
                    // The engine only lets through plain https hosts.
                    val host = URI(r.url).host
                    val url = if (r.secret.isEmpty()) r.url else secret(r.secret, host)?.let { key ->
                        val sep = if ('?' in r.url) '&' else '?'
                        "${r.url}$sep${URLEncoder.encode(r.param, UTF_8)}=${URLEncoder.encode(key, UTF_8)}"
                    }
                    val reply = if (url == null) Reply(0, "no ${r.secret} to ask with") else withContext(io) { fetch(url) }
                    if (r.secret.isNotEmpty() && (reply.status == 401 || reply.status == 403)) forget(r.secret, host)
                    val at = here(now(timezone))
                    val got = withContext(io) { pack.received(at, r, reply.status.toUShort(), reply.body) }
                    store.putAll(got)
                    stamp(got.keys)
                }
            } finally {
                syncing -= modules.toSet()
            }
            keep()
            redraw()
        }
    }

    /** The trip as text for another phone holding this pack: its facts, when each was kept, its files. */
    suspend fun send(): String? {
        val pack = pack ?: return null
        val (facts, kept) = store.toMap() to stamps.toMap()
        return withContext(io) { pack.share(facts, kept, files) }
    }

    /** What this phone would take of `sent`, a trip another phone shared. Refused when it is not a trip, or another pack's. */
    suspend fun weigh(sent: String): Taken? {
        val pack = pack ?: return null
        val (facts, kept) = store.toMap() to stamps.toMap()
        return withContext(io) { pack.take(sent, facts, kept) }
    }

    /**
     * Keeps the facts `taken` brings, then its files over the pack's when they load. `done` gets
     * why the files were not taken, or null.
     */
    fun take(taken: Taken, done: (String?) -> Unit) {
        viewModelScope.launch {
            store.putAll(taken.facts)
            // A stamp from past now would win over every later edit here, and travel on from here.
            val real = System.currentTimeMillis()
            stamps.putAll(taken.stamps.mapValues { minOf(it.value, real) })
            keep()
            val next = files + taken.files
            val why =
                try {
                    if (taken.files.isNotEmpty()) {
                        withContext(io) {
                            load("pack.yaml", next).close()
                            // The manifest last: it says when the pack was updated, so one written
                            // half way is sent again.
                            val order = taken.files.entries.sortedBy { it.key == "pack.yaml" }
                            for ((path, text) in order) if (files[path] != text) write(path, text)
                        }
                    }
                    null
                } catch (e: LoadException.Unreadable) {
                    e.detail
                } catch (e: LoadException.Invalid) {
                    e.errors.joinToString("\n") { it.message }
                } catch (e: IOException) {
                    "not written: ${e.message}"
                } catch (e: SecurityException) {
                    "not written: the folder can no longer be written. Open it again from the menu."
                }
            _state.value = attempt { open(if (why == null) next else files) }
            watch()
            ring()
            done(why)
        }
    }

    /** What is still to ring, on the real clock: the clock set by hand never sets an alarm. */
    private fun ring() {
        val pack = pack ?: return
        // Only the latest ask is handed on, so an older list never lands last.
        ringing?.cancel()
        ringing = viewModelScope.launch {
            val all =
                try {
                    withContext(io) { pack.alarms(here(now(timezone))) }
                } catch (_: CallException.Refused) {
                    return@launch
                }
            alarms(all)
        }
    }

    /** One timer, until the moment the engine said its answer changes, and any sync now due. */
    private fun watch() {
        refresh("")
        timer?.cancel()
        val until = (_state.value as? PackState.Showing)?.view?.watch?.until
        if (until.isNullOrEmpty()) return
        // Measured on the zone's own clock, so a daylight saving change in between counts.
        val wait = Duration.between(clock().atZone(timezone), LocalDateTime.parse(until).atZone(timezone)).toMillis()
        timer = viewModelScope.launch {
            delay(wait)
            redraw()
        }
    }

    /** The screen again, for the same pack in a changed world: a position, or the clock set by hand. */
    fun redraw() = update { pack ->
        val world = here()
        withContext(io) { pack.screen(world) }
    }

    private fun clock() = now(timezone).plusSeconds(shift)

    /** When `keys` were kept, on the real clock, as another phone compares them. */
    private fun stamp(keys: Set<String>) {
        val at = System.currentTimeMillis()
        for (k in keys) stamps[k] = at
    }

    /** What actions kept and when, saved under the pack's id. */
    private suspend fun keep() {
        val (kept, at) = store.toMap() to stamps.mapValues { Value.Number(it.value.toDouble()) }
        disk.withLock {
            withContext(io) {
                facts?.write(packId, encodeFacts(kept))
                facts?.write("$packId.stamps", encodeFacts(at))
            }
        }
    }
}

/** What the host keeps of each pack between runs: one file per pack id in `dir`. */
class Facts(private val dir: File, private val ext: String = "yaml") {
    fun read(id: String): String? = File(dir, "$id.$ext").takeIf { it.exists() }?.readText()

    fun write(id: String, text: String) {
        dir.mkdirs()
        File(dir, "$id.$ext").replace(text.encodeToByteArray())
    }
}

/** Writes `bytes` whole or not at all: a crash midway leaves the old file as it was. */
fun File.replace(bytes: ByteArray) {
    val next = File(parentFile, "$name$NEXT")
    next.writeBytes(bytes)
    Files.move(next.toPath(), toPath(), StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE)
}

/** The suffix of a file being written, never read as one. */
const val NEXT = ".next"

/** One stop of the day's route, as `map.route` hands it over. */
data class Stop(val lat: Double, val lon: Double)

/** A position as the engine reads it: `{lat, lon}`. */
fun point(lat: Double, lon: Double): Value =
    Value.Fields(listOf(Field("lat", Value.Number(lat)), Field("lon", Value.Number(lon))))

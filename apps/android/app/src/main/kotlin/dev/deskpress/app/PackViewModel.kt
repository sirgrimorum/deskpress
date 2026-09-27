package dev.deskpress.app

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dev.deskpress.engine.LoadedPack
import dev.deskpress.engine.Value
import dev.deskpress.engine.View
import dev.deskpress.engine.load
import java.time.Duration
import java.time.LocalDateTime
import java.time.ZoneId
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
) : ViewModel(scope) {
    private val _state = MutableStateFlow<PackState>(PackState.Loading)
    val state: StateFlow<PackState> = _state.asStateFlow()

    private var pack: LoadedPack? = null
    private var zone: ZoneId = ZoneId.of("UTC")
    private var warnings = 0
    // What actions kept. In memory until the store persists.
    private val store = mutableMapOf<String, Value>()
    private var timer: Job? = null

    init {
        viewModelScope.launch {
            _state.value = attempt {
                val loaded = withContext(io) { load("pack.yaml", files()) }
                addCloseable(loaded)
                pack = loaded
                zone = ZoneId.of(loaded.timezone())
                warnings = loaded.warnings().size
                val world = world(now(zone), store)
                show(withContext(io) { loaded.screen(world) }, warnings)
            }
            watch()
        }
    }

    /** Runs an action of the screen, as a tap on a node that names it. */
    fun act(action: String, arg: Value) = update { pack ->
        val world = world(now(zone), store)
        val out = withContext(io) { pack.dispatch(world, action, arg) }
        store.putAll(out.store)
        out.view
    }

    private fun update(call: suspend (LoadedPack) -> View) {
        val pack = pack ?: return
        viewModelScope.launch {
            _state.value = attempt { show(call(pack), warnings) }
            watch()
        }
    }

    /** One timer, until the moment the engine said its answer changes. */
    private fun watch() {
        timer?.cancel()
        val until = (_state.value as? PackState.Showing)?.view?.watch?.until
        if (until.isNullOrEmpty()) return
        val wait = Duration.between(now(zone), LocalDateTime.parse(until)).toMillis()
        timer = viewModelScope.launch {
            delay(wait)
            update { pack ->
                val world = world(now(zone), store)
                withContext(io) { pack.screen(world) }
            }
        }
    }
}

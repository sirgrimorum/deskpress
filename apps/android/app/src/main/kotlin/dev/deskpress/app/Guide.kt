package dev.deskpress.app

import android.annotation.SuppressLint
import android.content.Context
import android.location.Location
import android.net.ConnectivityManager
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.deskpress.engine.Node
import dev.deskpress.engine.Value
import java.io.IOException
import kotlin.math.roundToInt
import kotlinx.coroutines.flow.StateFlow
import org.maplibre.android.MapLibre
import org.maplibre.android.camera.CameraUpdateFactory
import org.maplibre.android.geometry.LatLng
import org.maplibre.android.geometry.LatLngBounds
import org.maplibre.android.location.LocationComponentActivationOptions
import org.maplibre.android.location.LocationComponentOptions
import org.maplibre.android.location.OnCameraTrackingChangedListener
import org.maplibre.android.location.modes.CameraMode
import org.maplibre.android.location.modes.RenderMode
import org.maplibre.android.maps.MapLibreMap
import org.maplibre.android.maps.MapView
import org.maplibre.android.maps.Style
import org.maplibre.android.offline.OfflineGeometryRegionDefinition
import org.maplibre.android.offline.OfflineManager
import org.maplibre.android.offline.OfflineRegion
import org.maplibre.android.offline.OfflineRegionError
import org.maplibre.android.offline.OfflineRegionStatus
import org.maplibre.geojson.MultiPolygon
import org.maplibre.geojson.Point
import org.maplibre.geojson.Polygon

/** The map style kept offline, OpenFreeMap's: no key, no cookies, no limits (decision 0031). */
private const val STYLE = "https://tiles.openfreemap.org/styles/liberty"

/**
 * Keeps the map of `cells` offline, beside what was kept before until this is whole. `done` hears
 * the bytes kept or why not; the returned stop gives up and deletes what this one kept.
 */
fun keep(context: Context, cells: Set<Cell>, progress: (Float) -> Unit, done: (Result<Long>) -> Unit): () -> Unit {
    if (context.getSystemService(ConnectivityManager::class.java).activeNetwork == null) {
        done(Result.failure(IOException("there is no network")))
        return {}
    }
    val squares = cells.map { c -> Polygon.fromLngLats(listOf(c.inset().map { Point.fromLngLat(it.first, it.second) })) }
    val area = OfflineGeometryRegionDefinition(STYLE, MultiPolygon.fromPolygons(squares), 10.0, 14.0, context.resources.displayMetrics.density, false)
    var region: OfflineRegion? = null
    var over = false
    val end = {
        over = true
        region?.setDownloadState(OfflineRegion.STATE_INACTIVE)
        MapLibre.setConnected(false)
    }
    val fail = { r: OfflineRegion, why: String ->
        if (!over) {
            end()
            drop(context, r)
            done(Result.failure(IOException(why)))
        }
    }
    // The only time the map library may use the network.
    MapLibre.setConnected(true)
    OfflineManager.getInstance(context).createOfflineRegion(
        area,
        ByteArray(0),
        object : OfflineManager.CreateOfflineRegionCallback {
            override fun onCreate(offlineRegion: OfflineRegion) {
                region = offlineRegion
                if (over) return drop(context, offlineRegion)
                offlineRegion.setObserver(
                    object : OfflineRegion.OfflineRegionObserver {
                        override fun onStatusChanged(status: OfflineRegionStatus) {
                            if (over) return
                            if (status.isComplete) {
                                end()
                                forget(context, offlineRegion.id) { done(Result.success(status.completedResourceSize)) }
                            } else if (status.requiredResourceCount > 0) {
                                progress(status.completedResourceCount.toFloat() / status.requiredResourceCount)
                            }
                        }

                        // The library tries a dropped connection or a busy server again, not a missing style.
                        override fun onError(error: OfflineRegionError) {
                            if (error.reason in setOf(OfflineRegionError.REASON_NOT_FOUND, OfflineRegionError.REASON_OTHER)) fail(offlineRegion, error.message)
                        }

                        override fun mapboxTileCountLimitExceeded(limit: Long) = fail(offlineRegion, "more than $limit tiles")
                    },
                )
                offlineRegion.setDownloadState(OfflineRegion.STATE_ACTIVE)
            }

            override fun onError(error: String) {
                end()
                done(Result.failure(IOException(error)))
            }
        },
    )
    return {
        if (!over) {
            end()
            region?.let { drop(context, it) }
        }
    }
}

/** Deletes the maps kept offline, all but the one with id `except`, and their tiles, then says so. */
fun forget(context: Context, except: Long? = null, done: () -> Unit = {}) {
    OfflineManager.getInstance(context).listOfflineRegions(
        object : OfflineManager.ListOfflineRegionsCallback {
            override fun onList(offlineRegions: Array<OfflineRegion>?) {
                val old = offlineRegions.orEmpty().filter { it.id != except }
                var left = old.size
                if (left == 0) return sweep(context, done)
                val one = {
                    left -= 1
                    if (left == 0) sweep(context, done)
                }
                old.forEach { it.gone(one) }
            }

            override fun onError(error: String) = done()
        },
    )
}

/** Deletes a region no keep finished, and its tiles. */
private fun drop(context: Context, region: OfflineRegion) = region.gone { sweep(context) }

/** Deletes the region, then goes on whether it went or not. */
private fun OfflineRegion.gone(then: () -> Unit) =
    delete(
        object : OfflineRegion.OfflineRegionDeleteCallback {
            override fun onDelete() = then()

            override fun onError(error: String) = then()
        },
    )

/** A deleted region's tiles stay in the library's ambient cache until it is cleared. */
private fun sweep(context: Context, done: () -> Unit = {}) =
    OfflineManager.getInstance(context).clearAmbientCache(
        object : OfflineManager.FileSourceCallback {
            override fun onSuccess() = done()

            override fun onError(message: String) = done()
        },
    )

private val Pin.at get() = LatLng(lat, lon)

/**
 * A Map card full screen on a real map, to walk by: the kept tiles when there are any, else plain
 * paper. The device's dot and heading, and a line from it to the pin picked under the map.
 */
@SuppressLint("MissingPermission")
@Composable
fun Guide(node: Node, kept: Boolean, position: StateFlow<Pair<Double, Double>?>, close: () -> Unit) {
    BackHandler(onBack = close)
    val tokens = LocalTokens.current
    val context = LocalContext.current
    val pins = remember(node) { (node.props["points"] as? Value.Items)?.items.orEmpty().mapNotNull(::pin) }
    val path = remember(node) { (node.props["path"] as? Value.Items)?.items.orEmpty().mapNotNull(::pin) }
    val here by position.collectAsStateWithLifecycle()
    var picked by remember { mutableStateOf<Pin?>(null) }
    var following by remember { mutableStateOf(false) }
    var map by remember { mutableStateOf<MapLibreMap?>(null) }
    // Bumped on every camera move and resize, so what is drawn over the map moves with it.
    var moved by remember { mutableIntStateOf(0) }
    val view = remember { MapView(context) }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    DisposableEffect(lifecycle) {
        view.onCreate(null)
        val watch = LifecycleEventObserver { _, event ->
            when (event) {
                Lifecycle.Event.ON_START -> view.onStart()
                Lifecycle.Event.ON_RESUME -> view.onResume()
                Lifecycle.Event.ON_PAUSE -> view.onPause()
                Lifecycle.Event.ON_STOP -> view.onStop()
                else -> {}
            }
        }
        lifecycle.addObserver(watch)
        onDispose {
            lifecycle.removeObserver(watch)
            if (lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) view.onPause()
            if (lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED)) view.onStop()
            view.onDestroy()
        }
    }
    val ground = tokens.color("card")
    LaunchedEffect(view) {
        view.getMapAsync { m ->
            m.setMinZoomPreference(10.0)
            m.setMaxZoomPreference(19.0)
            m.addOnCameraMoveListener { moved += 1 }
            view.addOnLayoutChangeListener { _, _, _, _, _, _, _, _, _ -> moved += 1 }
            m.setStyle(if (kept) Style.Builder().fromUri(STYLE) else Style.Builder().fromJson(paper(ground.toArgb()))) { style ->
                m.locationComponent.run {
                    // The host's fixes stop while nobody moves, so the dot never greys as stale.
                    val options = LocationComponentOptions.builder(context).enableStaleState(false).build()
                    activateLocationComponent(LocationComponentActivationOptions.Builder(context, style).useDefaultLocationEngine(false).locationComponentOptions(options).build())
                    isLocationComponentEnabled = true
                    renderMode = RenderMode.COMPASS
                    addOnCameraTrackingChangedListener(
                        object : OnCameraTrackingChangedListener {
                            override fun onCameraTrackingDismissed() {
                                following = false
                            }

                            override fun onCameraTrackingChanged(currentMode: Int) {}
                        },
                    )
                }
                // Framed once the style is in, when the view has its size.
                val spots = pins.map { it.at }.distinct()
                m.moveCamera(
                    if (spots.size < 2) CameraUpdateFactory.newLatLngZoom(spots.first(), 17.0)
                    else CameraUpdateFactory.newLatLngBounds(LatLngBounds.Builder().includes(spots).build(), 120),
                )
                map = m
            }
        }
    }
    LaunchedEffect(map, here) {
        val at = here ?: return@LaunchedEffect
        map?.locationComponent?.forceLocationUpdate(Location("deskpress").apply { latitude = at.first; longitude = at.second })
    }
    LaunchedEffect(map, following) { map?.locationComponent?.cameraMode = if (following) CameraMode.TRACKING_COMPASS else CameraMode.NONE }

    Scaffold(topBar = { Header(word(node, "open"), close) }, containerColor = tokens.color("paper")) { padding ->
        Column(Modifier.fillMaxSize().padding(padding)) {
            BoxWithConstraints(Modifier.weight(1f).fillMaxWidth()) {
                AndroidView({ view }, Modifier.fillMaxSize())
                val m = map
                if (m != null) {
                    val on = { at: LatLng -> moved.let { m.projection.toScreenLocation(at) }.let { Offset(it.x, it.y) } }
                    val ink = tokens.color("ink")
                    val action = tokens.color("action-bg")
                    Canvas(Modifier.fillMaxSize()) {
                        for (i in 1 until path.size) drawLine(ink, on(path[i - 1].at), on(path[i].at), 2.dp.toPx())
                        val to = picked
                        val from = here
                        if (to != null && from != null) {
                            drawLine(action, on(LatLng(from.first, from.second)), on(to.at), 3.dp.toPx(), pathEffect = PathEffect.dashPathEffect(floatArrayOf(18f, 12f)))
                        }
                    }
                    for (pin in pins) {
                        val chosen = pin == picked
                        val p = on(pin.at)
                        // A pin on the right half has its name on the dot's left, so the name stays on the map.
                        val flip = p.x > constraints.maxWidth / 2
                        Row(
                            Modifier.layout { measurable, limits ->
                                val row = measurable.measure(limits)
                                val dot = 7.dp.roundToPx()
                                layout(row.width, row.height) {
                                    row.place(p.x.roundToInt() + if (flip) dot - row.width else -dot, p.y.roundToInt() - row.height / 2)
                                }
                            }.clickable(role = Role.Button) { picked = pin },
                            horizontalArrangement = Arrangement.spacedBy(tokens.size("spacing.gap-xs")),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            val dot = @Composable { Box(Modifier.size(14.dp).clip(CircleShape).background(if (chosen) action else ink)) }
                            if (!flip) dot()
                            Text(pin.name, Modifier.background(tokens.color("card").copy(alpha = 0.8f)), style = style("label", if (chosen) "ink" else "ink-muted"))
                            if (flip) dot()
                        }
                    }
                }
            }
            Column(
                Modifier.padding(tokens.size("spacing.margin")),
                verticalArrangement = Arrangement.spacedBy(tokens.size("spacing.gap-s")),
            ) {
                Text(toward(picked, here, word(node, "pick"), word(node, "unplaced")), style = style("body"))
                Row(
                    Modifier.horizontalScroll(rememberScrollState()),
                    horizontalArrangement = Arrangement.spacedBy(tokens.size("spacing.gap-s")),
                ) {
                    Chip(word(node, "follow"), following) { following = !following }
                    choices(pins).forEach { pin -> Chip(pin.name, pin == picked) { picked = pin } }
                }
            }
        }
    }
}

package dev.deskpress.app

import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import dev.deskpress.engine.Region
import java.time.Duration
import java.time.LocalDateTime
import java.time.temporal.ChronoUnit

/**
 * What the shell remembers about this phone, whatever pack is open. `shift` moves the clock by
 * that many seconds and `at` pins the position as "lat,lon": both for trying a moment out.
 */
data class Shell(val scale: Float = 1f, val navigator: String = "", val shift: Long = 0, val at: String = "")

val LocalShell = staticCompositionLocalOf { Shell() }

/** The text sizes offered, each a multiplier of whatever the theme asked for. */
val SCALES = listOf("Small" to 0.85f, "Normal" to 1f, "Large" to 1.2f, "Largest" to 1.45f)

/** The steps the pretend clock moves by, in seconds. */
private val STEPS = listOf("−1 day" to -86_400L, "−1 h" to -3_600L, "+1 h" to 3_600L, "+1 day" to 86_400L)

/**
 * What "Settings" shows: the choices the shell makes, not the pack. `maps` are the phone's own
 * apps that open a map, `places` the pack's regions a pretend position can be put in, `offline`
 * the line about the maps kept and `offers` the buttons under it.
 */
@Composable
fun Settings(
    shell: Shell,
    now: () -> LocalDateTime,
    maps: List<Pair<String, String>>,
    places: List<Region>,
    offline: String,
    offers: List<Pair<String, () -> Unit>>,
    set: (Shell) -> Unit,
    close: () -> Unit,
) {
    val tokens = LocalTokens.current
    Scaffold(topBar = { Header("SETTINGS", close) }, containerColor = tokens.color("paper")) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).verticalScroll(rememberScrollState()).padding(tokens.size("spacing.margin")),
            verticalArrangement = Arrangement.spacedBy(tokens.size("spacing.gap")),
        ) {
            Text("TEXT SIZE", style = style("label", "ink-muted"))
            Choice(SCALES.map { it.first }, SCALES.indexOfFirst { it.second == shell.scale }) {
                set(shell.copy(scale = SCALES[it].second))
            }
            Text("MAPS OPEN IN", style = style("label", "ink-muted"))
            val apps = listOf("Ask" to "") + maps
            // An app since removed is the phone's own choice again, so "Ask" is what is on.
            Choice(apps.map { it.first }, apps.indexOfFirst { it.second == shell.navigator }.coerceAtLeast(0), across = false) {
                set(shell.copy(navigator = apps[it].second))
            }
            Text("MAPS OFFLINE", style = style("label", "ink-muted"))
            Text(offline, style = style("body"))
            if (offers.isNotEmpty()) Choice(offers.map { it.first }, -1) { offers[it].second() }
            Moment(shell, now, set)
            Text("PRETEND TO BE AT", style = style("label", "ink-muted"))
            val spots = listOf("Where I really am" to "") + places.map { it.id to "${it.lat},${it.lon}" }
            Choice(spots.map { it.first }, spots.indexOfFirst { it.second == shell.at }, across = false) {
                set(shell.copy(at = spots[it].second))
            }
        }
    }
}

/** The pretend clock: the moment it shows, typed or stepped, and the way back to the real one. */
@Composable
private fun Moment(shell: Shell, now: () -> LocalDateTime, set: (Shell) -> Unit) {
    val shown = remember(shell.shift) { now().plusSeconds(shell.shift).truncatedTo(ChronoUnit.MINUTES).toString() }
    var typed by remember(shell.shift) { mutableStateOf(shown) }
    Text(if (shell.shift == 0L) "THE MOMENT: NOW" else "THE MOMENT: PRETEND", style = style("label", "ink-muted"))
    OutlinedTextField(
        typed,
        { typed = it },
        Modifier.fillMaxWidth(),
        singleLine = true,
        isError = parse(typed) == null,
        supportingText = { Text("YYYY-MM-DDTHH:MM, the pack's own time") },
    )
    Choice(listOf("Set", "Back to now"), -1) {
        if (it == 1) set(shell.copy(shift = 0))
        // Set on the time as shown keeps the clock as it is.
        else if (typed != shown) parse(typed)?.let { at -> set(shell.copy(shift = Duration.between(now(), at).seconds)) }
    }
    Choice(STEPS.map { it.first }, -1) { set(shell.copy(shift = shell.shift + STEPS[it].second)) }
}

private fun parse(text: String): LocalDateTime? = runCatching { LocalDateTime.parse(text.trim()) }.getOrNull()

/** Options of equal width, the one at `on` filled: in one row, or one under the other. */
@Composable
private fun Choice(labels: List<String>, on: Int, across: Boolean = true, pick: (Int) -> Unit) {
    val tokens = LocalTokens.current
    val shape = RoundedCornerShape(tokens.size("radius.control"))
    val framed = Modifier.fillMaxWidth().border(tokens.size("border.base"), tokens.color("line"), shape).clip(shape)
    if (across) {
        Row(framed) { labels.forEachIndexed { i, label -> Option(label, i == on) { pick(i) } } }
    } else {
        Column(framed) { labels.forEachIndexed { i, label -> Row { Option(label, i == on) { pick(i) } } } }
    }
}

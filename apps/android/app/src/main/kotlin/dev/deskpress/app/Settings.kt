package dev.deskpress.app

import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip

/** What the shell remembers about this phone, whatever pack is open. */
data class Shell(val scale: Float = 1f, val navigator: String = "")

val LocalShell = staticCompositionLocalOf { Shell() }

/** The text sizes offered, each a multiplier of whatever the theme asked for. */
val SCALES = listOf("Small" to 0.85f, "Normal" to 1f, "Large" to 1.2f, "Largest" to 1.45f)

/** The map apps offered. An empty package lets the phone ask, and that is the default. */
val NAVIGATORS =
    listOf(
        "Ask" to "",
        "Google Maps" to "com.google.android.apps.maps",
        "Organic Maps" to "app.organicmaps",
        "OsmAnd" to "net.osmand",
    )

/** What "Settings" shows: the two choices the shell makes, not the pack. */
@Composable
fun Settings(shell: Shell, set: (Shell) -> Unit, close: () -> Unit) {
    val tokens = LocalTokens.current
    Scaffold(topBar = { Header("SETTINGS", close) }, containerColor = tokens.color("paper")) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).padding(tokens.size("spacing.margin")),
            verticalArrangement = Arrangement.spacedBy(tokens.size("spacing.gap")),
        ) {
            Text("TEXT SIZE", style = style("label", "ink-muted"))
            Choice(SCALES.map { it.first }, SCALES.indexOfFirst { it.second == shell.scale }) {
                set(shell.copy(scale = SCALES[it].second))
            }
            Text("MAPS OPEN IN", style = style("label", "ink-muted"))
            Choice(NAVIGATORS.map { it.first }, NAVIGATORS.indexOfFirst { it.second == shell.navigator }) {
                set(shell.copy(navigator = NAVIGATORS[it].second))
            }
        }
    }
}

/** One row of options of equal width, the one at `on` filled. */
@Composable
private fun Choice(labels: List<String>, on: Int, pick: (Int) -> Unit) {
    val tokens = LocalTokens.current
    val shape = RoundedCornerShape(tokens.size("radius.control"))
    Row(
        Modifier.fillMaxWidth()
            .border(tokens.size("border.base"), tokens.color("line"), shape)
            .clip(shape),
    ) {
        labels.forEachIndexed { i, label -> Option(label, i == on) { pick(i) } }
    }
}

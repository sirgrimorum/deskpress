package dev.deskpress.app

import android.app.Activity
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.layout.onPlaced
import androidx.compose.ui.layout.positionInParent
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.coerceIn
import androidx.compose.ui.zIndex
import androidx.core.view.WindowCompat
import dev.deskpress.engine.Finding
import dev.deskpress.engine.Node
import dev.deskpress.engine.Value
import kotlin.math.roundToInt
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** What the shell offers on every screen, whatever the pack draws. */
class Menu(
    val open: () -> Unit,
    val check: () -> Unit,
    val design: () -> Unit,
    val settings: () -> Unit,
    val send: () -> Unit,
    val take: () -> Unit,
)

/** The bytes of a file inside the pack, for the pictures a pack carries. */
val LocalPackFile = staticCompositionLocalOf<(String) -> ByteArray?> { { null } }

/** Opens a Map card full screen on a real map, where the host has one (decision 0031). */
val LocalGuide = staticCompositionLocalOf<((Node) -> Unit)?> { null }

/** Whether an action would store anything, asked of the engine while a drag is held (decision 0035). */
val LocalWould = staticCompositionLocalOf<suspend (String, Value) -> Boolean> { { _, _ -> true } }

/** The id of the theme drawn for the holder, if the file has one, and its tokens. */
@Composable
fun holder(state: PackState): Pair<String?, Tokens> {
    val dark = isSystemInDarkTheme()
    val scale = LocalShell.current.scale
    val shown = state as? PackState.Showing
    val theme = shown?.theme ?: Value.Null
    val wanted = shown?.view?.tree?.theme.orEmpty()
    val kid = shown?.view?.tree?.kid == true
    return remember(theme, wanted, kid, dark, scale) {
        val id = pick(theme, wanted, dark)
        id to tokens(theme, id, dark, kid).scaled(scale)
    }
}

/** The screen the engine chose, in the theme the holder asked for, under `pretend` when it says something. */
@Composable
fun PackScreen(state: PackState, act: (String, Value) -> Unit, menu: Menu, pretend: String) {
    val shown = state as? PackState.Showing
    val tokens = holder(state).second
    CompositionLocalProvider(LocalTokens provides tokens) {
        val nodes = shown?.view?.tree?.nodes.orEmpty()
        val frame = remember(nodes) { nodes.firstOrNull { it.kind == "Screen" } }
        // A Dialog sits over the list, not in it: a lazy item below the fold is never drawn.
        val (dialogs, body) = remember(nodes, frame) { nodes.filter { it !== frame }.partition { it.kind == "Dialog" } }
        // The status bar icons sit on the top bar, or on the paper when there is none.
        val behind = tokens.color(if (frame != null) "bar-bg" else "paper")
        val view = LocalView.current
        SideEffect {
            val window = (view.context as? Activity)?.window ?: return@SideEffect
            WindowCompat.getInsetsController(window, view).isAppearanceLightStatusBars =
                behind.luminance() > 0.5f
        }
        Scaffold(
            topBar = {
                Column {
                    Bar(frame, act, menu)
                    if (pretend.isNotEmpty()) Pretend(pretend, menu.settings.takeUnless { tokens.kid })
                }
            },
            bottomBar = { if (frame != null) Foot(frame) },
            containerColor = tokens.color("paper"),
        ) { padding ->
            val inside = Modifier.fillMaxSize().padding(padding)
            when (state) {
                PackState.Loading ->
                    Box(inside, contentAlignment = Alignment.Center) {
                        CircularProgressIndicator(color = tokens.color("ink"))
                    }
                is PackState.Failed -> Findings(state.reason, state.errors, inside)
                is PackState.Showing ->
                    // A screen of its own opens at the top: keys repeat across screens.
                    key(state.view.tree.screen) {
                        LazyColumn(
                            inside.padding(horizontal = tokens.size("spacing.margin")),
                            verticalArrangement = Arrangement.spacedBy(tokens.size("spacing.gap")),
                        ) {
                            item { Box(Modifier.height(tokens.size("spacing.pad-top"))) }
                            items(body, key = { it.key }) { Draw(it, act) }
                            item { Box(Modifier.height(tokens.size("spacing.pad-bottom"))) }
                        }
                        dialogs.forEach { Draw(it, act) }
                    }
            }
        }
    }
}

/** A strip that says the moment or the place is pretend, so it is not left on by mistake. */
@Composable
private fun Pretend(text: String, tap: (() -> Unit)?) {
    val tokens = LocalTokens.current
    Text(
        if (tap != null) "$text Tap to change." else text,
        Modifier.fillMaxWidth()
            .background(tokens.color("alert"))
            .then(if (tap != null) Modifier.minimumInteractiveComponentSize().clickable(role = Role.Button, onClick = tap) else Modifier)
            .padding(horizontal = tokens.size("spacing.margin"), vertical = tokens.size("spacing.gap-xs")),
        style = style("label", "paper"),
    )
}

/** What "Check the pack" shows: whether it loads, and every finding the validator made. */
@Composable
fun Check(state: PackState, close: () -> Unit) {
    val tokens = LocalTokens.current
    val (head, findings) =
        when (state) {
            PackState.Loading -> "…" to emptyList()
            is PackState.Failed -> state.reason to state.errors
            is PackState.Showing -> "loads. ${state.warnings.size} warnings." to state.warnings
        }
    Scaffold(topBar = { Header("CHECK THE PACK", close) }, containerColor = tokens.color("paper")) { padding ->
        Findings(head, findings, Modifier.fillMaxSize().padding(padding))
    }
}

/** The top bar of a shell screen: back, and its title. */
@Composable
internal fun Header(title: String, close: () -> Unit) {
    val tokens = LocalTokens.current
    Row(
        Modifier.fillMaxWidth()
            .background(tokens.color("bar-bg"))
            .statusBarsPadding()
            .padding(horizontal = tokens.size("spacing.margin")),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Round("←", "Back", close)
        Text(title, Modifier.weight(1f), style = style("moment", "bar-ink"))
    }
}

@Composable
private fun Findings(head: String, findings: List<Finding>, modifier: Modifier) {
    val tokens = LocalTokens.current
    LazyColumn(modifier.padding(tokens.size("spacing.margin"))) {
        item { Text(head, style = style("title")) }
        items(findings) { Line("", it.message, it.at) }
    }
}

/** The top bar: back as a round button, the title, the other events as pills, and the menu. */
@Composable
private fun Bar(frame: Node?, act: (String, Value) -> Unit, menu: Menu) {
    val tokens = LocalTokens.current
    if (frame == null) {
        if (!tokens.kid) {
            Box(Modifier.fillMaxWidth().statusBarsPadding(), contentAlignment = Alignment.TopEnd) {
                More(menu, "ink")
            }
        }
        return
    }
    val prop = { key: String -> frame.props[key].text() }
    Row(
        Modifier.fillMaxWidth()
            .background(tokens.color("bar-bg"))
            .statusBarsPadding()
            .padding(start = tokens.size("spacing.margin")),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(tokens.size("spacing.gap-s")),
    ) {
        frame.on["back"]?.let { Round("←", prop("back"), { act(it, Value.Null) }) }
        Text(
            prop("title").uppercase(),
            Modifier.weight(1f).padding(vertical = tokens.size("spacing.pad-y")),
            style = style("moment", "bar-ink"),
        )
        frame.on.filterKeys { it != "back" }.forEach { (event, action) ->
            Pill(prop(event)) { act(action, Value.Null) }
        }
        if (!tokens.kid) More(menu, "bar-ink")
    }
}

@Composable
private fun Round(glyph: String, label: String, tap: () -> Unit) {
    val tokens = LocalTokens.current
    Box(
        Modifier.minimumInteractiveComponentSize()
            .size(40.dp)
            .border(tokens.size("border.base"), tokens.color("bar-ink"), CircleShape)
            .clip(CircleShape)
            .clickable(role = Role.Button, onClick = tap)
            .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        Text(glyph, style = style("value", "bar-ink"))
    }
}

@Composable
private fun Pill(label: String, tap: () -> Unit) {
    val tokens = LocalTokens.current
    val shape = RoundedCornerShape(tokens.size("radius.pill"))
    Box(
        Modifier.minimumInteractiveComponentSize()
            .clip(shape)
            .background(tokens.color("bar-ink"))
            .clickable(role = Role.Button, onClick = tap)
            .padding(horizontal = tokens.size("spacing.gap"), vertical = tokens.size("spacing.gap-xs")),
    ) {
        Text(label, style = style("label", "bar-bg"))
    }
}

/** A pack's chip, or one the full-screen map picks with; `on` fills it with the action colours. */
@Composable
internal fun Chip(label: String, on: Boolean, tap: (() -> Unit)?) {
    val tokens = LocalTokens.current
    Box(
        Modifier.heightIn(min = tokens.size("touch.chip"))
            .clip(RoundedCornerShape(tokens.size("radius.chip")))
            .background(tokens.color(if (on) "action-bg" else "chip-bg"))
            .then(if (tap != null) Modifier.clickable(role = Role.Button, onClick = tap) else Modifier)
            .padding(horizontal = tokens.size("spacing.gap")),
        contentAlignment = Alignment.Center,
    ) {
        Text(label, style = style("label", if (on) "action-ink" else "chip-ink"))
    }
}

@Composable
private fun More(menu: Menu, color: String) {
    var open by remember { mutableStateOf(false) }
    Box {
        Box(
            Modifier.minimumInteractiveComponentSize()
                .clickable(role = Role.Button) { open = true }
                .semantics { contentDescription = "Menu" },
            contentAlignment = Alignment.Center,
        ) {
            Text("⋮", style = style("title", color))
        }
        DropdownMenu(open, { open = false }) {
            DropdownMenuItem({ Text("Open a pack…") }, { open = false; menu.open() })
            DropdownMenuItem({ Text("Check the pack") }, { open = false; menu.check() })
            DropdownMenuItem({ Text("Design system") }, { open = false; menu.design() })
            DropdownMenuItem({ Text("Settings") }, { open = false; menu.settings() })
            DropdownMenuItem({ Text("Send the trip…") }, { open = false; menu.send() })
            DropdownMenuItem({ Text("Take a trip sent to you…") }, { open = false; menu.take() })
        }
    }
}

/** The bottom bar: what comes next, from the Screen's `foot_label`, `foot_time` and `foot`. */
@Composable
private fun Foot(frame: Node) {
    val tokens = LocalTokens.current
    val prop = { key: String -> frame.props[key].text() }
    if (prop("foot_label").isEmpty() && prop("foot").isEmpty()) return
    Column(
        Modifier.fillMaxWidth()
            .background(tokens.color("bar-bg"))
            .navigationBarsPadding()
            .padding(horizontal = tokens.size("spacing.margin"), vertical = tokens.size("spacing.pad-y")),
    ) {
        if (prop("foot_label").isNotEmpty()) {
            Text(prop("foot_label").uppercase(), style = style("label", "bar-muted"))
        }
        Row(horizontalArrangement = Arrangement.spacedBy(tokens.size("spacing.gap-s"))) {
            if (prop("foot_time").isNotEmpty()) Text(prop("foot_time"), style = style("value", "bar-ink"))
            Text(prop("foot"), style = style("next", "bar-ink"))
        }
    }
}

/**
 * One node of the closed set. A kind this renderer does not know draws nothing. A node with a
 * `tap` runs that action, with the node's `value`.
 */
@Composable
internal fun Draw(node: Node, act: (String, Value) -> Unit) {
    val tokens = LocalTokens.current
    val prop = { key: String -> node.props[key].text() }
    val tap = node.on["tap"]?.let { action -> { act(action, node.props["value"] ?: Value.Null) } }
    val touch =
        if (tap != null) Modifier.clickable(role = Role.Button, onClick = tap) else Modifier
    when (node.kind) {
        "BigValue" -> {
            val text = prop("text")
            // The hero steps down a size when the answer is long, so it stays on one or two lines.
            val step = if (text.length <= 5) "hero" else if (text.length <= 12) "hero-m" else "hero-s"
            Column {
                Text(text, style = style(step))
                if (prop("caption").isNotEmpty()) Text(prop("caption"), style = style("body"))
            }
        }
        // Only the outline of a pack with no rules has one.
        "Title" -> Text(prop("text"), style = style("title"))
        "Label" -> Text(prop("text"), touch, style = style("body-s", "ink-muted"))
        "Card" -> Box(touch) { Boxed(prop("title"), prop("text"), prop("caption"), node.props["kid"] == Value.Bool(true)) }
        "Alert" -> {
            val shape = RoundedCornerShape(tokens.size("radius.card"))
            Column(
                Modifier.fillMaxWidth()
                    .border(tokens.size("border.base"), tokens.color("alert"), shape)
                    .clip(shape)
                    .background(tokens.color("card"))
                    .then(touch)
                    .padding(tokens.size("spacing.pad-x")),
                verticalArrangement = Arrangement.spacedBy(tokens.size("spacing.gap-xs")),
            ) {
                if (prop("title").isNotEmpty()) Text(prop("title"), style = style("value", "alert"))
                if (prop("text").isNotEmpty()) Text(prop("text"), style = style("body"))
            }
        }
        "Missing" -> Missing(prop("title"), prop("text"), prop("caption"))
        "Button" -> {
            val shape = RoundedCornerShape(tokens.size(if (tokens.kid) "radius.kid" else "radius.action"))
            val shadow = if (tokens.kid) tokens.shadows["kid-action"] else null
            Box(
                Modifier.fillMaxWidth()
                    .padding(end = (shadow?.x ?: 0f).dp, bottom = (shadow?.y ?: 0f).dp)
                    .hard(shadow, shape)
                    .heightIn(min = tokens.size("touch.action-height"))
                    .clip(shape)
                    .background(tokens.color("action-bg"))
                    .then(touch),
                contentAlignment = Alignment.Center,
            ) {
                Text(prop("label"), Modifier.padding(tokens.size("spacing.pad-x")), style = style("action", "action-ink"))
            }
        }
        "Chip" -> Row { Chip(prop("text"), false, tap) }
        "Row" -> Line(prop("time"), prop("text"), prop("caption"), prop("state"), tap)
        "PhraseRow" ->
            Row(
                Modifier.fillMaxWidth().then(touch).padding(vertical = tokens.size("spacing.gap-s")),
                horizontalArrangement = Arrangement.spacedBy(tokens.size("spacing.gap")),
            ) {
                Column(Modifier.width(110.dp)) {
                    if (prop("hint").isNotEmpty()) Text(prop("hint").uppercase(), style = style("label", "ink-muted"))
                    Text(prop("text"), style = style("body").copy(fontWeight = FontWeight.Bold))
                }
                Text(prop("translation"), Modifier.weight(1f), style = style("body"))
            }
        "Segmented" -> Segmented(node, act)
        "Map" -> Paper(node)
        "Day" -> Day(node, act)
        "Check" -> Ticked(prop("text"), prop("caption"), node.props["checked"] == Value.Bool(true), tap)
        "Field" -> Typed(node, act)
        "Group" -> Grouped(prop("title"), node.children, act)
        "Dialog" -> Popup(prop("title"), prop("text"), prop("close")) { node.on["close"]?.let { act(it, Value.Null) } }
    }
}

/** A Dialog: the title small, the text large, over the screen until it is closed. */
@Composable
private fun Popup(title: String, text: String, label: String, close: () -> Unit) {
    val tokens = LocalTokens.current
    AlertDialog(
        onDismissRequest = close,
        title = { Text(title, style = style("body", "ink-muted")) },
        text = { Text(text, Modifier.verticalScroll(rememberScrollState()), style = style("title")) },
        confirmButton = { TextButton(close) { Text(label.ifEmpty { "✕" }, style = style("action", "ink")) } },
        containerColor = tokens.color("card"),
    )
}

/** A Group: a titled box around the nodes it holds, each drawn as it would be outside. */
@Composable
private fun Grouped(title: String, children: List<Node>, act: (String, Value) -> Unit) {
    val tokens = LocalTokens.current
    val shape = RoundedCornerShape(tokens.size("radius.card"))
    Column(
        Modifier.fillMaxWidth()
            .border(tokens.size("border.base"), tokens.color("card-line"), shape)
            .clip(shape)
            .background(tokens.color("card"))
            .padding(horizontal = tokens.size("spacing.pad-x"), vertical = tokens.size("spacing.pad-y")),
        verticalArrangement = Arrangement.spacedBy(tokens.size("spacing.gap-s")),
    ) {
        if (title.isNotEmpty()) Text(title.uppercase(), style = style("label", "ink-muted"))
        children.forEach { key(it.key) { Draw(it, act) } }
    }
}

/** A Card: a titled box. In kid mode, with `kid: true`, the thick kid box with its inner shadow. */
@Composable
private fun Boxed(title: String, text: String, caption: String, kid: Boolean) {
    val tokens = LocalTokens.current
    val child = kid && tokens.kid
    val shape = RoundedCornerShape(tokens.size(if (child) "radius.kid" else "radius.card"))
    Column(
        Modifier.fillMaxWidth()
            .border(tokens.size(if (child) "border.kid" else "border.base"), tokens.color("card-line"), shape)
            .clip(shape)
            .background(tokens.color("card"))
            .hard(if (child) tokens.shadows["kid-box"] else null, shape)
            .padding(horizontal = tokens.size("spacing.pad-x"), vertical = tokens.size("spacing.pad-y")),
        verticalArrangement = Arrangement.spacedBy(tokens.size("spacing.gap-xs")),
    ) {
        if (title.isNotEmpty()) Text(title.uppercase(), style = style("label", "ink-muted"))
        if (text.isNotEmpty()) Text(text, style = style("body"))
        if (caption.isNotEmpty()) Text(caption, style = style("body-s", "ink-muted"))
    }
}

/** A fact nobody confirmed: a striped hole with a dashed edge. Nothing to tap. */
@Composable
private fun Missing(title: String, text: String, caption: String) {
    val tokens = LocalTokens.current
    val radius = tokens.size("radius.card")
    val edge = tokens.color("missing-border")
    val width = tokens.size("border.base")
    Column(
        Modifier.fillMaxWidth()
            .clip(RoundedCornerShape(radius))
            .background(tokens.color("missing-fill"))
            .drawBehind {
                val gap = 12.dp.toPx()
                var x = -size.height
                while (x < size.width) {
                    drawLine(edge.copy(alpha = 0.18f), Offset(x, size.height), Offset(x + size.height, 0f), 2.dp.toPx())
                    x += gap
                }
                val line = width.toPx()
                drawRoundRect(
                    edge,
                    topLeft = Offset(line / 2, line / 2),
                    size = size.copy(size.width - line, size.height - line),
                    cornerRadius = CornerRadius(radius.toPx()),
                    style = Stroke(line, pathEffect = PathEffect.dashPathEffect(floatArrayOf(8.dp.toPx(), 6.dp.toPx()))),
                )
            }
            .padding(horizontal = tokens.size("spacing.pad-x"), vertical = tokens.size("spacing.pad-y")),
        verticalArrangement = Arrangement.spacedBy(tokens.size("spacing.gap-xs")),
    ) {
        if (title.isNotEmpty()) Text(title.uppercase(), style = style("label", "missing-text"))
        Text(text, style = style("body", "missing-text"))
        // A BigValue waiting for confirmation comes here as Missing, caption and all.
        if (caption.isNotEmpty()) Text(caption, style = style("body-s", "missing-text"))
    }
}

/**
 * A list line with a rule under it: the time in a fixed column, the text, the caption. `now`
 * and `picked` stand out, `past` steps back, and `locked` puts the time in the alert color.
 */
@Composable
private fun Line(
    time: String,
    text: String,
    caption: String = "",
    state: String = "",
    tap: (() -> Unit)? = null,
) {
    val tokens = LocalTokens.current
    // `picked` is the hour being moved about, and stands out the same way the current one does.
    val now = state == "now" || state == "picked"
    val ink = if (now) "highlight-ink" else if (state == "past") "ink-muted" else "ink"
    val rule = tokens.color("rule")
    val thin = tokens.size("border.rule")
    Row(
        Modifier.fillMaxWidth()
            .then(if (now) Modifier.background(tokens.color("highlight-bg")) else Modifier)
            .drawBehind {
                drawLine(rule, Offset(0f, size.height), Offset(size.width, size.height), thin.toPx())
                if (now) drawLine(tokens.color("highlight-line"), Offset(0f, 0f), Offset(0f, size.height), 8.dp.toPx())
            }
            // A row to read has no minimum; one to tap is a full touch target.
            .then(
                if (tap == null) Modifier
                else Modifier.clickable(role = Role.Button, onClick = tap).heightIn(min = tokens.size("touch.row-height"))
            )
            .padding(horizontal = tokens.size("spacing.gap-s"), vertical = tokens.size("spacing.gap-s")),
        horizontalArrangement = Arrangement.spacedBy(tokens.size("spacing.gap-s")),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (time.isNotEmpty()) {
            Text(time, Modifier.width(66.dp), style = style("value", if (state == "locked") "alert" else ink))
        }
        Column(Modifier.weight(1f)) {
            Text(text, style = style("body", ink))
            if (caption.isNotEmpty()) Text(caption, style = style("body-s", "ink-muted"))
        }
    }
}

/** Minutes to a dp of a Day's hours, and the tallest one gets: an overnight flight stays a row. */
private const val DP_A_MINUTE = 0.8f
private val TALLEST = 240.dp

/** A drag held on a Day: the row and its span, the edge (none for the whole block), where it began, the axis then. */
private class Drag(val row: Int, val span: Span, val edge: String?, val action: String, val event: Value, val from: Float, val spans: List<Span>) {
    var offset by mutableFloatStateOf(0f)
    val minutes by derivedStateOf { draggedBy(spans, span, edge, from + offset) }
    /** The engine's answer for the step held now; none while it is asked. */
    var fits by mutableStateOf<Boolean?>(null)
}

/** The tint of a landing that fits; one that does not is the alert color. */
private val FITS = Color(0xFF2E7D32)

/**
 * The Day (decisions 0032, 0035): each hour as tall as it lasts, the page its time axis. A handle
 * drags an hour, the picked one's edges drag its start and end, and a placeholder shows the landing.
 */
@Composable
private fun Day(node: Node, act: (String, Value) -> Unit) {
    val tokens = LocalTokens.current
    val would = LocalWould.current
    val density = LocalDensity.current
    val items = (node.props["blocks"] as? Value.Items)?.items.orEmpty()
    val picked = node.props["picked"].text()
    val word = node.props["move"].text().ifEmpty { "Move" }
    val steps =
        listOf("earlier" to "15 minutes earlier", "later" to "15 minutes later", "shorter" to "15 minutes shorter", "longer" to "15 minutes longer")
            .map { (key, said) -> node.props[key].text().ifEmpty { said } }
    val moving = node.on["move"]
    val resizing = node.on["resize"]
    val haptic = LocalHapticFeedback.current
    val spans = remember(items) { arrayOfNulls<Span>(items.size) }
    var drag by remember(items) { mutableStateOf<Drag?>(null) }
    val held = drag
    val by = held?.minutes ?: 0
    LaunchedEffect(held, by) {
        if (held == null || by == 0) return@LaunchedEffect
        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
        held.fits = null
        held.fits = would(held.action, dragged(held.event, by, held.edge))
    }
    // Only the drag a gesture began ends it: a second finger never drops the first one's.
    fun drop(mine: Drag?) {
        if (mine == null || drag !== mine) return
        drag = null
        if (mine.minutes != 0) act(mine.action, dragged(mine.event, mine.minutes, mine.edge))
    }
    val rule = tokens.color("rule")
    val thin = tokens.size("border.rule")
    Box(Modifier.fillMaxWidth()) {
        Column(Modifier.fillMaxWidth()) {
            items.forEachIndexed { i, item ->
                val event = item.field("event") ?: Value.Null
                val leg = item.field("leg") == Value.Bool(true)
                val time = item.field("time").text()
                val timed = time.isNotEmpty() && !leg
                val real = item.field("state").text()
                val chosen = timed && picked.isNotEmpty() && event.text() == picked
                val state = if (chosen) "picked" else real
                val now = state == "now" || state == "picked"
                val late = leg && item.field("late") == Value.Bool(true)
                val ink = if (late) "alert" else if (leg || state == "past") "ink-muted" else if (now) "highlight-ink" else "ink"
                val tap = node.on["tap"]?.takeIf { timed }?.let { a -> { act(a, event) } }
                // A locked hour never changes; one begun keeps its start; one over keeps both.
                val fixed = item.field("locked") == Value.Bool(true) || real == "past"
                val begun = real == "now"
                val move = moving?.takeIf { timed && !fixed && !begun }
                val resize = resizing?.takeIf { chosen && !fixed }
                val least = if (leg) tokens.size("touch.min") / 2 else if (timed) tokens.size("touch.row-height") else 0.dp
                val minutes = (item.field("lasts") as? Value.Number)?.value?.toFloat() ?: 0f
                val start = clock(time)
                // A drag on this row from `from` px down the Day, against the axis as it stands; none while another is held.
                fun Modifier.drags(edge: String?, action: String, from: (Span) -> Float) =
                    pointerInput(items, i, edge) {
                        var mine: Drag? = null
                        detectVerticalDragGestures(
                            onDragStart = {
                                val s = spans[i]
                                mine = if (drag != null || s == null) null else Drag(i, s, edge, action, event, from(s), spans.filterNotNull())
                                mine?.let { drag = it }
                            },
                            onDragEnd = { drop(mine) },
                            onDragCancel = { if (drag === mine) drag = null },
                        ) { change, dy -> change.consume(); mine?.let { it.offset += dy } }
                    }
                Box(
                    Modifier.fillMaxWidth()
                        .onPlaced {
                            val top = it.positionInParent().y
                            spans[i] = start?.let { at -> Span(top, it.size.height.toFloat(), at, minutes.toInt()) }
                        }
                        // The dragged row over the rest, and the picked one over its neighbours, whose edges its grips straddle.
                        .then(
                            if (held?.row == i && held.edge == null) Modifier.zIndex(2f).graphicsLayer { translationY = held.offset }
                            else if (chosen) Modifier.zIndex(1f)
                            else Modifier
                        ),
                ) {
                    Row(
                        Modifier.fillMaxWidth()
                            .heightIn(min = (minutes * DP_A_MINUTE).dp.coerceIn(least, maxOf(least, TALLEST)))
                            .then(if (now) Modifier.background(tokens.color("highlight-bg")) else Modifier)
                            .drawBehind {
                                drawLine(rule, Offset(0f, size.height), Offset(size.width, size.height), thin.toPx())
                                if (now) drawLine(tokens.color("highlight-line"), Offset(0f, 0f), Offset(0f, size.height), 8.dp.toPx())
                            }
                            // TalkBack's way to the same edits a drag sends, fifteen minutes at a time.
                            .then(
                                if (!chosen) Modifier
                                else Modifier.semantics {
                                    customActions =
                                        listOfNotNull(
                                            move?.let { a -> CustomAccessibilityAction(steps[0]) { act(a, dragged(event, -15)); true } },
                                            move?.let { a -> CustomAccessibilityAction(steps[1]) { act(a, dragged(event, 15)); true } },
                                            resize?.let { a -> CustomAccessibilityAction(steps[2]) { act(a, dragged(event, -15, "end")); true } },
                                            resize?.let { a -> CustomAccessibilityAction(steps[3]) { act(a, dragged(event, 15, "end")); true } },
                                        )
                                }
                            )
                            .then(if (tap != null) Modifier.clickable(role = Role.Button, onClick = tap) else Modifier)
                            .padding(horizontal = tokens.size("spacing.gap-s"), vertical = tokens.size("spacing.gap-s")),
                        horizontalArrangement = Arrangement.spacedBy(tokens.size("spacing.gap-s")),
                    ) {
                        Text(time, Modifier.width(66.dp), style = style(if (leg) "body-s" else "value", if (real == "locked") "alert" else ink))
                        Column(Modifier.weight(1f)) {
                            Text(item.field("text").text(), style = style(if (leg) "body-s" else "body", ink))
                            val caption = item.field("duration").text()
                            if (caption.isNotEmpty()) Text(caption, style = style("body-s", if (late) "alert" else "ink-muted"))
                        }
                        if (move != null) {
                            Box(
                                Modifier.size(tokens.size("touch.min"))
                                    // A node of its own, not merged into the row: tests and readers find the handle.
                                    .semantics(mergeDescendants = true) { contentDescription = "$word ${item.field("text").text()}" }
                                    .drags(null, move) { s -> s.top },
                                contentAlignment = Alignment.Center,
                            ) { Text("≡", style = style("value", "ink-muted")) }
                        }
                    }
                    if (resize != null) {
                        for (edge in listOf("start", "end")) {
                            if (edge == "start" && begun) continue
                            val grip = tokens.size("touch.min") / 2
                            Box(
                                Modifier.align(if (edge == "start") Alignment.TopCenter else Alignment.BottomCenter)
                                    .offset(y = if (edge == "start") -grip / 2 else grip / 2)
                                    .size(tokens.size("touch.min"), grip)
                                    .drags(edge, resize) { s -> if (edge == "start") s.top else s.top + s.height },
                                contentAlignment = Alignment.Center,
                            ) {
                                Box(Modifier.size(32.dp, 4.dp).clip(CircleShape).background(tokens.color("ink-muted")))
                            }
                        }
                    }
                }
            }
        }
        // Where the drag lands, tinted by the engine's answer and dashed when it does not fit; nothing while
        // it reads as no change. An edge dragged past the other still shows a sliver.
        if (held != null && by != 0) {
            val (top, bottom) = landing(held.spans, held.span, held.edge, by).let { (a, b) -> minOf(a, b) to maxOf(a, b) }
            val fits = held.fits
            val tint = when (fits) { true -> FITS; false -> tokens.color("alert"); null -> rule }
            Box(
                Modifier.fillMaxWidth()
                    .offset { IntOffset(0, top.roundToInt()) }
                    .height(with(density) { (bottom - top).toDp() }.coerceAtLeast(thin * 4))
                    .background(tint.copy(alpha = 0.2f))
                    .drawBehind {
                        val dash = if (fits == false) PathEffect.dashPathEffect(floatArrayOf(6.dp.toPx(), 4.dp.toPx())) else null
                        drawRect(tint, style = Stroke(thin.toPx(), pathEffect = dash))
                    },
            )
        }
    }
}

/** How wide a pin is, and the share of the map its scale line measures. */
private val DOT = 10.dp
private const val SCALE = 0.25f

/** A number under a pin or a path, or nothing. */
private fun fraction(v: Value?, key: String): Float =
    (v.field(key) as? Value.Number)?.value?.toFloat() ?: 0f

/**
 * A map the pack carries (decision 0026): pins where the pack puts them, joined in the order the
 * pack gives, over its own picture when there is one. Pins with a position open a real map.
 */
@Composable
private fun Paper(node: Node) {
    val tokens = LocalTokens.current
    val pins = (node.props["points"] as? Value.Items)?.items.orEmpty()
    val guide = LocalGuide.current?.takeIf { pins.any { pin(it) != null } }
    val path = (node.props["path"] as? Value.Items)?.items.orEmpty()
    val name = node.props["image"].text()
    val read = LocalPackFile.current
    // Read and decoded off the main thread: the picture is whatever size the pack shipped.
    val picture =
        produceState<Bitmap?>(null, name, read) {
            value =
                if (name.isEmpty()) null
                else withContext(Dispatchers.IO) {
                    read(name)?.let { BitmapFactory.decodeByteArray(it, 0, it.size) }
                }
        }.value
    val shape = RoundedCornerShape(tokens.size("radius.card"))
    Column(
        if (guide != null) Modifier.clickable(role = Role.Button) { guide(node) } else Modifier,
        verticalArrangement = Arrangement.spacedBy(tokens.size("spacing.gap-xs")),
    ) {
        BoxWithConstraints(
            Modifier.fillMaxWidth()
                .aspectRatio(picture?.let { it.width.toFloat() / it.height } ?: 1.4f)
                .border(tokens.size("border.base"), tokens.color("line"), shape)
                .clip(shape)
                .background(tokens.color("card")),
        ) {
            if (picture != null) {
                Image(picture.asImageBitmap(), null, Modifier.fillMaxSize(), contentScale = ContentScale.FillBounds)
            }
            if (path.size > 1) {
                val ink = tokens.color("line")
                Canvas(Modifier.fillMaxSize()) {
                    val at = { p: Value -> Offset(fraction(p, "x") * size.width, fraction(p, "y") * size.height) }
                    for (i in 1 until path.size) drawLine(ink, at(path[i - 1]), at(path[i]), DOT.toPx() / 5f)
                }
            }
            for (pin in pins) {
                val now = pin.field("state").text() == "now"
                // A pin past the middle writes its name leftwards, so no label runs off the paper.
                val x = fraction(pin, "x")
                val left = x <= 0.5f
                Row(
                    Modifier.align(if (left) Alignment.TopStart else Alignment.TopEnd)
                        .offset(
                            if (left) maxWidth * x - DOT / 2 else DOT / 2 - maxWidth * (1f - x),
                            maxHeight * fraction(pin, "y") - DOT / 2,
                        ),
                    horizontalArrangement = Arrangement.spacedBy(tokens.size("spacing.gap-xs")),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    if (!left) Named(pin, now)
                    Box(Modifier.size(DOT).clip(CircleShape).background(tokens.color(if (now) "action-bg" else "ink")))
                    if (left) Named(pin, now)
                }
            }
        }
        val span = (node.props["span"] as? Value.Number)?.value ?: 0.0
        if (span > 0.0) {
            Row(
                horizontalArrangement = Arrangement.spacedBy(tokens.size("spacing.gap-xs")),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Box(
                    Modifier.fillMaxWidth(SCALE)
                        .height(tokens.size("border.base"))
                        .background(tokens.color("ink-muted")),
                )
                Text(metres(span * SCALE), style = style("label", "ink-muted"))
            }
        }
        if (node.props["caption"].text().isNotEmpty()) {
            Text(node.props["caption"].text(), style = style("body-s", "ink-muted"))
        }
        if (guide != null) Text(word(node, "open"), style = style("label", "ink"))
    }
}

/** What a pin is called, beside its dot. */
@Composable
private fun Named(pin: Value, now: Boolean) =
    Text(pin.field("name").text(), style = style("label", if (now) "ink" else "ink-muted"))

/** A line to tick off: the box, then what it says and its caption. The whole row is the touch target. */
@Composable
private fun Ticked(text: String, caption: String, done: Boolean, tap: (() -> Unit)?) {
    val tokens = LocalTokens.current
    val box = RoundedCornerShape(tokens.size("radius.control"))
    Row(
        Modifier.fillMaxWidth()
            .then(if (tap != null) Modifier.clickable(role = Role.Checkbox, onClick = tap) else Modifier)
            .heightIn(min = tokens.size("touch.row-height"))
            .padding(horizontal = tokens.size("spacing.gap-s")),
        horizontalArrangement = Arrangement.spacedBy(tokens.size("spacing.gap-s")),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            Modifier.size(tokens.size("touch.chip"))
                .border(tokens.size("border.base"), tokens.color("line"), box)
                .clip(box)
                .then(if (done) Modifier.background(tokens.color("action-bg")) else Modifier),
            contentAlignment = Alignment.Center,
        ) {
            if (done) Text("\u2713", style = style("value", "action-ink"))
        }
        Column(Modifier.weight(1f)) {
            Text(text, style = style("body", if (done) "ink-muted" else "ink"))
            if (caption.isNotEmpty()) Text(caption, style = style("body-s", "ink-muted"))
        }
    }
}

/**
 * A line to type on. The typing is local so the caret never jumps; every keystroke goes out as the
 * node's `on_change`, and the pack keeps it in the screen's own state.
 */
@Composable
private fun Typed(node: Node, act: (String, Value) -> Unit) {
    val tokens = LocalTokens.current
    val prop = { key: String -> node.props[key].text() }
    var text by remember(node.on["change"]) { mutableStateOf(prop("value")) }
    OutlinedTextField(
        text,
        { typed -> text = typed; node.on["change"]?.let { act(it, Value.Text(typed)) } },
        Modifier.fillMaxWidth(),
        singleLine = true,
        label = { Text(prop("label"), style = style("label", "ink-muted")) },
        placeholder = { Text(prop("hint"), style = style("body", "ink-muted")) },
        textStyle = style("body"),
        shape = RoundedCornerShape(tokens.size("radius.control")),
    )
}

/** Two or three options of equal width; the one equal to `value` is filled. A tap sends its item. */
@Composable
private fun Segmented(node: Node, act: (String, Value) -> Unit) {
    val tokens = LocalTokens.current
    val items = (node.props["items"] as? Value.Items)?.items.orEmpty()
    val chosen = node.props["value"]
    val shape = RoundedCornerShape(tokens.size("radius.control"))
    Row(
        Modifier.fillMaxWidth()
            .border(tokens.size("border.base"), tokens.color("line"), shape)
            .clip(shape),
    ) {
        items.forEach { item ->
            val on = item == chosen
            Option(
                name(item),
                on,
                node.on["tap"]?.let { action -> { act(action, item) } },
            )
        }
    }
}

@Composable
internal fun RowScope.Option(label: String, on: Boolean, tap: (() -> Unit)?) {
    val tokens = LocalTokens.current
    Box(
        Modifier.weight(1f)
            .heightIn(min = tokens.size("touch.min"))
            .then(if (on) Modifier.background(tokens.color("action-bg")) else Modifier)
            .then(if (tap != null) Modifier.clickable(role = Role.RadioButton, onClick = tap) else Modifier),
        contentAlignment = Alignment.Center,
    ) {
        Text(label, style = style("label", if (on) "action-ink" else "ink"))
    }
}

/** An item as an option: a mapping by its name, title or id; anything else as its text. */
private fun name(item: Value): String {
    val fields = (item as? Value.Fields)?.fields ?: return item.text()
    return listOf("name", "title", "id").firstNotNullOfOrNull { key ->
        fields.firstOrNull { it.key == key }?.value?.text()?.takeIf { it.isNotEmpty() }
    }.orEmpty()
}

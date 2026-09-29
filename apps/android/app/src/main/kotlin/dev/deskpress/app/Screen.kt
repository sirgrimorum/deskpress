package dev.deskpress.app

import android.app.Activity
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.dp
import androidx.core.view.WindowCompat
import dev.deskpress.engine.Finding
import dev.deskpress.engine.Node
import dev.deskpress.engine.Value

/** What the shell offers on every screen, whatever the pack draws. */
class Menu(val open: () -> Unit, val check: () -> Unit, val design: () -> Unit)

/** The id of the theme drawn for the holder, if the file has one, and its tokens. */
@Composable
fun holder(state: PackState): Pair<String?, Tokens> {
    val dark = isSystemInDarkTheme()
    val shown = state as? PackState.Showing
    val theme = shown?.theme ?: Value.Null
    val wanted = shown?.view?.tree?.theme.orEmpty()
    val kid = shown?.view?.tree?.kid == true
    return remember(theme, wanted, kid, dark) {
        val id = pick(theme, wanted, dark)
        id to tokens(theme, id, dark, kid)
    }
}

/** The screen the engine chose, in the theme the holder asked for. */
@Composable
fun PackScreen(state: PackState, act: (String, Value) -> Unit, menu: Menu) {
    val shown = state as? PackState.Showing
    val tokens = holder(state).second
    CompositionLocalProvider(LocalTokens provides tokens) {
        val nodes = shown?.view?.tree?.nodes.orEmpty()
        val frame = nodes.firstOrNull { it.kind == "Screen" }
        // The status bar icons sit on the top bar, or on the paper when there is none.
        val behind = tokens.color(if (frame != null) "bar-bg" else "paper")
        val view = LocalView.current
        SideEffect {
            val window = (view.context as? Activity)?.window ?: return@SideEffect
            WindowCompat.getInsetsController(window, view).isAppearanceLightStatusBars =
                behind.luminance() > 0.5f
        }
        Scaffold(
            topBar = { Bar(frame, act, menu) },
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
                    LazyColumn(
                        inside.padding(horizontal = tokens.size("spacing.margin")),
                        verticalArrangement = Arrangement.spacedBy(tokens.size("spacing.gap")),
                    ) {
                        item { Box(Modifier.height(tokens.size("spacing.pad-top"))) }
                        items(nodes.filter { it !== frame }) { Draw(it, act) }
                        item { Box(Modifier.height(tokens.size("spacing.pad-bottom"))) }
                    }
            }
        }
    }
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
        "Chip" -> Row {
            val shape = RoundedCornerShape(tokens.size("radius.chip"))
            Box(
                Modifier.heightIn(min = tokens.size("touch.chip"))
                    .clip(shape)
                    .background(tokens.color("chip-bg"))
                    .then(touch)
                    .padding(horizontal = tokens.size("spacing.gap")),
                contentAlignment = Alignment.Center,
            ) {
                Text(prop("text"), style = style("label", "chip-ink"))
            }
        }
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
        "Group" -> Grouped(prop("title"), node.children, act)
    }
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
        children.forEach { Draw(it, act) }
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
 * stands out, `past` steps back, and `locked` puts the time in the alert color.
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
    val now = state == "now"
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
private fun RowScope.Option(label: String, on: Boolean, tap: (() -> Unit)?) {
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

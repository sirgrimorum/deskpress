package dev.deskpress.app

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import dev.deskpress.engine.Edit
import dev.deskpress.engine.Node
import dev.deskpress.engine.Value

/**
 * An edit of the theme file: the path of the value, the new text, and who hears the outcome, with
 * the edited file when it could not be written.
 */
typealias ThemeEdit = (path: List<String>, value: String, done: (String?, Edit?) -> Unit) -> Unit

/** One of each component, drawn in the tokens shown above them. */
private val SAMPLES =
    listOf(
        node("BigValue", "text" to "09:30", "caption" to "BigValue"),
        node("Card", "title" to "Card", "text" to "Text on a card", "caption" to "a caption"),
        node("Alert", "title" to "Alert", "text" to "Something to know before going"),
        node("Row", "time" to "10:00", "text" to "Row", "caption" to "now", "state" to "now"),
        node("Row", "time" to "08:00", "text" to "Row", "caption" to "past", "state" to "past"),
        node("Chip", "text" to "Chip"),
        node("Button", "label" to "Button"),
        node("Missing", "title" to "Missing", "text" to "Not in the pack yet"),
        node("Group", "title" to "Group").copy(
            children = listOf(node("Row", "text" to "Row in a group", "caption" to "a caption"))
        ),
    )

private fun node(kind: String, vararg props: Pair<String, String>) =
    Node(kind, props.associate { (k, v) -> k to Value.Text(v) }, emptyMap(), emptyList())

/**
 * The design system of the pack, live: every token of the holder's theme and each component
 * drawn with them. A tap on a value edits it in the theme file; a refusal says why, and `save`
 * keeps a copy of a file that could not be written where the person picks.
 */
@Composable
fun Design(state: PackState, edit: ThemeEdit, save: (Edit) -> Unit, close: () -> Unit) {
    val (id, tokens) = holder(state)
    val theme = (state as? PackState.Showing)?.theme ?: Value.Null
    var editing by remember { mutableStateOf<List<String>?>(null) }
    // Without a theme of the file there is nothing to write to.
    val tap = { path: List<String> -> if (id != null) editing = path }
    CompositionLocalProvider(LocalTokens provides tokens) {
        Scaffold(topBar = { Header("DESIGN SYSTEM", close) }, containerColor = tokens.color("paper")) { padding ->
            LazyColumn(
                Modifier.fillMaxSize().padding(padding).padding(horizontal = tokens.size("spacing.margin")),
                verticalArrangement = Arrangement.spacedBy(tokens.size("spacing.gap-s")),
            ) {
                item {
                    val head = id ?: "The built-in theme: this pack has no theme file to edit."
                    Text(head, Modifier.padding(top = tokens.size("spacing.pad-top")), style = style("title"))
                }
                item { Heading("COLORS") }
                items(BUILT_IN.colors.keys.toList()) { name ->
                    val path = listOf("themes", id.orEmpty(), "colors", name)
                    val hex = theme.at(path).text().ifEmpty { tokens.color(name).hex() }
                    Token(name, hex, { tap(path) }) {
                        val shape = RoundedCornerShape(tokens.size("radius.control"))
                        Box(
                            Modifier.size(32.dp)
                                .border(tokens.size("border.rule"), tokens.color("line"), shape)
                                .padding(1.dp)
                                .background(tokens.color(name), shape)
                        )
                    }
                }
                item { Heading("TYPE") }
                items(theme.field("type").entries()) { (step, v) ->
                    val shown = "${v.field("size").text()} · ${v.field("weight").text()}"
                    val look = tokens.type[step]?.style(tokens.color("ink")) ?: style("body")
                    Token(step, shown, { tap(listOf("type", step, "size")) }) { Text("Aa", style = look) }
                }
                for (group in GROUPS) {
                    val sizes = theme.field(group).entries()
                    if (sizes.isEmpty()) continue
                    item { Heading(group.uppercase()) }
                    items(sizes) { (key, v) -> Token(key, v.text(), { tap(listOf(group, key)) }) {} }
                }
                item { Heading("COMPONENTS") }
                items(SAMPLES) { Draw(it) { _, _ -> } }
                item { Box(Modifier.padding(bottom = tokens.size("spacing.pad-bottom"))) }
            }
        }
        editing?.let { path -> Editor(path, theme.at(path).text(), edit, save) { editing = null } }
    }
}

private fun Value.at(path: List<String>): Value? = path.fold<String, Value?>(this) { v, key -> v.field(key) }

private fun androidx.compose.ui.graphics.Color.hex() = "#%06X".format(toArgb() and 0xFFFFFF)

@Composable
private fun Heading(text: String) {
    Text(
        text,
        Modifier.padding(top = LocalTokens.current.size("spacing.gap")),
        style = style("label", "ink-muted"),
    )
}

/** A token: what it looks like, its name, and its value as the file writes it. */
@Composable
private fun Token(name: String, value: String, tap: () -> Unit, look: @Composable () -> Unit) {
    val tokens = LocalTokens.current
    Row(
        Modifier.fillMaxWidth()
            .heightIn(min = tokens.size("touch.min"))
            .clickable(role = Role.Button, onClick = tap),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(tokens.size("spacing.gap")),
    ) {
        look()
        Text(name, Modifier.weight(1f), style = style("body"))
        Text(value, style = style("body-s", "ink-muted"))
    }
}

/**
 * One value of the theme file in a text field. Save keeps the dialog open on a refusal, which
 * offers a copy when the edit was valid but the file could not be written.
 */
@Composable
private fun Editor(path: List<String>, old: String, edit: ThemeEdit, save: (Edit) -> Unit, close: () -> Unit) {
    // The value comes selected, so typing replaces it.
    var text by remember(path) { mutableStateOf(TextFieldValue(old, TextRange(0, old.length))) }
    val focus = remember { FocusRequester() }
    LaunchedEffect(path) { focus.requestFocus() }
    var why by remember(path) { mutableStateOf<String?>(null) }
    var busy by remember(path) { mutableStateOf(false) }
    var copy by remember(path) { mutableStateOf<Edit?>(null) }
    AlertDialog(
        onDismissRequest = close,
        confirmButton = {
            TextButton(
                {
                    busy = true
                    edit(path, text.text) { refused, unwritten ->
                        busy = false
                        why = refused
                        copy = unwritten
                        if (refused == null) close()
                    }
                },
                enabled = !busy,
            ) { Text("Save") }
        },
        dismissButton = {
            Row {
                copy?.let { c ->
                    TextButton({
                        save(c)
                        close()
                    }) { Text("Save a copy") }
                }
                TextButton(close) { Text("Cancel") }
            }
        },
        title = { Text(path.last()) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    text,
                    { text = it },
                    Modifier.focusRequester(focus),
                    singleLine = true,
                    label = { Text(path.joinToString(".")) },
                )
                why?.let { Text(it, color = MaterialTheme.colorScheme.error) }
            }
        },
    )
}

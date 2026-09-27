package dev.deskpress.app

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import dev.deskpress.engine.Node
import dev.deskpress.engine.Value

@Composable
fun PackScreen(state: PackState, act: (String, Value) -> Unit) {
    Scaffold { padding ->
        val inside = Modifier.fillMaxSize().padding(padding)
        when (state) {
            PackState.Loading ->
                Box(inside, contentAlignment = Alignment.Center) { CircularProgressIndicator() }
            is PackState.Failed ->
                Column(inside.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(state.reason, style = MaterialTheme.typography.headlineSmall)
                    state.errors.forEach { Text("${it.at}: ${it.message}") }
                }
            is PackState.Showing ->
                LazyColumn(inside) { items(state.view.tree.nodes) { Draw(it, act) } }
        }
    }
}

/**
 * One node of the closed set. A kind this renderer does not know draws nothing. A node with a
 * `tap` runs that action, with the node's `value`; a Screen offers each of its events as a button
 * labelled by the prop of the same name.
 */
@Composable
private fun Draw(node: Node, act: (String, Value) -> Unit) {
    val prop = { key: String -> node.props[key].text() }
    val tap = node.on["tap"]?.let { action -> { act(action, node.props["value"] ?: Value.Null) } }
    val pad = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)
    val touch = if (tap != null) Modifier.clickable(role = Role.Button, onClick = tap) else Modifier
    when (node.kind) {
        "Screen" ->
            Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(
                    prop("title"),
                    Modifier.weight(1f),
                    style = MaterialTheme.typography.headlineMedium,
                )
                node.on.forEach { (event, action) ->
                    TextButton({ act(action, Value.Null) }) { Text(prop(event)) }
                }
            }
        "Title" ->
            Text(prop("text"), Modifier.padding(16.dp), style = MaterialTheme.typography.headlineMedium)
        "BigValue" ->
            Column(pad) {
                Text(prop("text"), style = MaterialTheme.typography.displayMedium)
                Text(prop("caption"), style = MaterialTheme.typography.bodyLarge)
            }
        "Row" -> {
            // The agenda marks each block: the one now stands out, the past ones step back.
            val state = prop("state")
            val colors =
                ListItemDefaults.colors(
                    containerColor =
                        if (state == "now") MaterialTheme.colorScheme.primaryContainer
                        else MaterialTheme.colorScheme.surface,
                    headlineColor =
                        if (state == "past") MaterialTheme.colorScheme.outline
                        else MaterialTheme.colorScheme.onSurface,
                )
            ListItem(
                headlineContent = { Text(prop("text")) },
                modifier = touch,
                supportingContent = { Text(prop("caption")) },
                colors = colors,
            )
        }
        "Label" -> Text(prop("text"), pad.then(touch))
        "Chip" -> AssistChip({ tap?.invoke() }, { Text(prop("text")) }, pad)
        "Card", "Missing", "Alert" -> {
            val scheme = MaterialTheme.colorScheme
            val color =
                when (node.kind) {
                    "Alert" -> scheme.errorContainer
                    "Missing" -> scheme.tertiaryContainer
                    else -> scheme.surfaceVariant
                }
            Card(pad.fillMaxWidth().then(touch), colors = CardDefaults.cardColors(color)) {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    if (prop("title").isNotEmpty()) {
                        Text(prop("title"), style = MaterialTheme.typography.titleSmall)
                    }
                    Text(prop("text"))
                    // A BigValue waiting for confirmation comes here as Missing, caption and all.
                    if (prop("caption").isNotEmpty()) Text(prop("caption"))
                }
            }
        }
        "Button" -> Button({ tap?.invoke() }, pad) { Text(prop("label")) }
    }
}

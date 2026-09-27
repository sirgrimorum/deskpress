package dev.deskpress.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
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

/** One node of the closed set. A kind this renderer does not know draws nothing. */
@Composable
private fun Draw(node: Node, act: (String, Value) -> Unit) {
    val prop = { key: String -> node.props[key].text() }
    val pad = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)
    when (node.kind) {
        "Screen", "Title" ->
            Text(
                prop(if (node.kind == "Screen") "title" else "text"),
                Modifier.padding(16.dp),
                style = MaterialTheme.typography.headlineMedium,
            )
        "BigValue" ->
            Column(pad) {
                Text(prop("text"), style = MaterialTheme.typography.displayMedium)
                Text(prop("caption"), style = MaterialTheme.typography.bodyLarge)
            }
        "Row" ->
            ListItem(
                headlineContent = { Text(prop("text")) },
                supportingContent = { Text(prop("caption")) },
            )
        "Label" -> Text(prop("text"), pad)
        "Alert" -> {
            val colors = CardDefaults.cardColors(MaterialTheme.colorScheme.errorContainer)
            Card(pad.fillMaxWidth(), colors = colors) { Text(prop("text"), Modifier.padding(16.dp)) }
        }
        "Button" ->
            Button(
                { node.on["tap"]?.let { act(it, node.props["value"] ?: Value.Null) } },
                pad,
            ) {
                Text(prop("label"))
            }
    }
}

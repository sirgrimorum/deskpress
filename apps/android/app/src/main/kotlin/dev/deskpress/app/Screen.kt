package dev.deskpress.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
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

@Composable
fun PackScreen(state: PackState) {
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
                LazyColumn(inside) { items(state.tree.nodes) { Draw(it) } }
        }
    }
}

/** One node of the closed set. A kind this renderer does not know draws nothing. */
@Composable
private fun Draw(node: Node) {
    val prop = { key: String -> node.props[key].orEmpty() }
    when (node.kind) {
        "Title" ->
            Text(
                prop("text"),
                Modifier.padding(16.dp),
                style = MaterialTheme.typography.headlineMedium,
            )
        "Row" ->
            ListItem(
                headlineContent = { Text(prop("text")) },
                supportingContent = { Text(prop("caption")) },
            )
    }
}

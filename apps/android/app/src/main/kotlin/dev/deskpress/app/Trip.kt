package dev.deskpress.app

import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import dev.deskpress.engine.Taken

/** Asks before a trip another phone sent is taken (decision 0034). */
@Composable
fun TakeTrip(taken: Taken, take: () -> Unit, dismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = dismiss,
        title = { Text("Take the trip") },
        text = { Text(brings(taken)) },
        confirmButton = { TextButton(take) { Text("Take") } },
        dismissButton = { TextButton(dismiss) { Text("Not now") } },
    )
}

/** What a trip sent brings, in a line: how many facts are newer there, and a newer pack. */
fun brings(taken: Taken): String {
    if (taken.facts.isEmpty() && taken.files.isEmpty()) return "Nothing there is newer than on this phone."
    val n = taken.facts.size
    val facts = if (n == 0) null else "$n newer ${if (n == 1) "fact" else "facts"} from the other phone"
    val pack = if (taken.files.isEmpty()) null else "the pack as updated ${taken.updated.replace('T', ' ')}"
    return listOfNotNull(facts, pack).joinToString(", and ").replaceFirstChar { it.uppercase() } + "."
}

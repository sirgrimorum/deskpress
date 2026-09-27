package dev.deskpress.app

import android.content.res.AssetManager
import dev.deskpress.engine.CallException
import dev.deskpress.engine.Finding
import dev.deskpress.engine.LoadException
import dev.deskpress.engine.Value
import dev.deskpress.engine.View
import dev.deskpress.engine.World
import java.io.IOException
import java.time.LocalDateTime
import java.time.format.DateTimeFormatter

/** The tree version this renderer draws. A newer engine tree is refused, not half drawn. */
const val TREE_VERSION = 2u

sealed interface PackState {
    data object Loading : PackState

    data class Failed(val reason: String, val errors: List<Finding> = emptyList()) : PackState

    data class Showing(val view: View, val warnings: Int) : PackState
}

/** Runs calls to the engine and turns every way they can fail into [PackState.Failed]. */
inline fun attempt(call: () -> PackState): PackState =
    try {
        call()
    } catch (e: LoadException.Unreadable) {
        PackState.Failed(e.detail)
    } catch (e: LoadException.Invalid) {
        PackState.Failed("${e.errors.size} errors", e.errors)
    } catch (e: CallException.Refused) {
        PackState.Failed(e.detail)
    } catch (e: IOException) {
        PackState.Failed(e.message ?: "the pack could not be read")
    }

/** The view to show, or a refusal when its tree is newer than this renderer. */
fun show(view: View, warnings: Int): PackState =
    if (view.tree.version != TREE_VERSION) {
        PackState.Failed("tree version ${view.tree.version}, this app draws $TREE_VERSION")
    } else {
        PackState.Showing(view, warnings)
    }

private val MINUTE = DateTimeFormatter.ofPattern("yyyy-MM-dd'T'HH:mm")

/** The world at `now`, in the pack's timezone. Holder and places come with their modules. */
fun world(now: LocalDateTime, store: Map<String, Value>) =
    World(now.format(MINUTE), "", emptyList(), store)

/** A value as text on screen: lists joined, a mapping or nothing as empty. */
fun Value?.text(): String =
    when (this) {
        is Value.Text -> value
        is Value.Number -> if (value % 1.0 == 0.0) value.toLong().toString() else value.toString()
        is Value.Bool -> value.toString()
        is Value.Items -> items.joinToString(", ") { it.text() }
        is Value.Fields, Value.Null, null -> ""
    }

/** Every file under `folder` in the app's assets, keyed by its path inside that folder. */
fun AssetManager.pack(folder: String): Map<String, String> {
    val files = mutableMapOf<String, String>()
    fun walk(dir: String, prefix: String) {
        for (name in list(dir).orEmpty()) {
            val path = "$dir/$name"
            if (list(path).isNullOrEmpty()) {
                files[prefix + name] = open(path).bufferedReader().use { it.readText() }
            } else {
                walk(path, "$prefix$name/")
            }
        }
    }
    walk(folder, "")
    return files
}

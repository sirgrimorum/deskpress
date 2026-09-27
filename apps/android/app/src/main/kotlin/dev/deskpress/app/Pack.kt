package dev.deskpress.app

import android.content.res.AssetManager
import dev.deskpress.engine.Finding
import dev.deskpress.engine.LoadException
import dev.deskpress.engine.Tree
import dev.deskpress.engine.load
import java.io.IOException

/** The tree version this renderer draws. A newer engine tree is refused, not half drawn. */
const val TREE_VERSION = 1u

sealed interface PackState {
    data object Loading : PackState

    data class Failed(val reason: String, val errors: List<Finding> = emptyList()) : PackState

    data class Showing(val tree: Tree, val warnings: Int) : PackState
}

/** Loads a pack from its files, keyed by path inside the pack folder. Never throws. */
fun open(files: () -> Map<String, String>): PackState =
    try {
        load("pack.yaml", files()).use { pack ->
            val tree = pack.screen()
            if (tree.version != TREE_VERSION) {
                PackState.Failed("tree version ${tree.version}, this app draws $TREE_VERSION")
            } else {
                PackState.Showing(tree, pack.warnings().size)
            }
        }
    } catch (e: LoadException.Unreadable) {
        PackState.Failed(e.detail)
    } catch (e: LoadException.Invalid) {
        PackState.Failed("${e.errors.size} errors", e.errors)
    } catch (e: IOException) {
        PackState.Failed(e.message ?: "the pack could not be read")
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

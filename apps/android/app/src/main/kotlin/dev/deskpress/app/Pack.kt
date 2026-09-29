package dev.deskpress.app

import android.content.Context
import android.content.res.AssetManager
import android.net.Uri
import androidx.documentfile.provider.DocumentFile
import dev.deskpress.engine.CallException
import dev.deskpress.engine.Finding
import dev.deskpress.engine.LoadException
import dev.deskpress.engine.Value
import dev.deskpress.engine.View
import dev.deskpress.engine.World
import java.io.File
import java.io.IOException
import java.time.LocalDateTime
import java.time.ZoneId
import java.time.format.DateTimeFormatter

/** The tree version this renderer draws. A newer engine tree is refused, not half drawn. */
const val TREE_VERSION = 4u

sealed interface PackState {
    data object Loading : PackState

    data class Failed(val reason: String, val errors: List<Finding> = emptyList()) : PackState

    data class Showing(
        val view: View,
        val warnings: List<Finding> = emptyList(),
        val theme: Value = Value.Null,
    ) : PackState
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
fun show(view: View, warnings: List<Finding> = emptyList(), theme: Value = Value.Null): PackState =
    if (view.tree.version != TREE_VERSION) {
        PackState.Failed("tree version ${view.tree.version}, this app draws $TREE_VERSION")
    } else {
        PackState.Showing(view, warnings, theme)
    }

private val MINUTE = DateTimeFormatter.ofPattern("yyyy-MM-dd'T'HH:mm")

/**
 * The world at `now`, in the pack's timezone, with the regions the device is `inside` when it is
 * `located`, and the local time now in each of the plan's other `zones`. The holder comes with its
 * module.
 */
fun world(
    now: LocalDateTime,
    store: Map<String, Value>,
    inside: List<String> = emptyList(),
    located: Boolean = false,
    zones: Map<String, LocalDateTime> = emptyMap(),
) = World(now.format(MINUTE), "", inside, located, store, zones.mapValues { it.value.format(MINUTE) })

/** A zone by its IANA name, or `fallback` for an empty or unknown one. */
fun zoneOr(name: String, fallback: ZoneId): ZoneId =
    if (name.isEmpty()) fallback else runCatching { ZoneId.of(name) }.getOrDefault(fallback)

/** A value as text on screen: lists joined, a mapping or nothing as empty. */
fun Value?.text(): String =
    when (this) {
        is Value.Text -> value
        is Value.Number -> if (value % 1.0 == 0.0) value.toLong().toString() else value.toString()
        is Value.Bool -> value.toString()
        is Value.Items -> items.joinToString(", ") { it.text() }
        is Value.Fields, Value.Null, null -> ""
    }

/** The text files a pack is made of. Anything else in its folder, like a PDF, is not read. */
private val TEXT = setOf("yaml", "yml", "json")

private fun isText(name: String) = name.substringAfterLast('.', "").lowercase() in TEXT

/** Every pack file under `folder` in the app's assets, keyed by its path inside that folder. */
fun AssetManager.pack(folder: String): Map<String, String> {
    val files = mutableMapOf<String, String>()
    fun walk(dir: String, prefix: String) {
        for (name in list(dir).orEmpty()) {
            val path = "$dir/$name"
            if (list(path).isNullOrEmpty()) {
                if (isText(name)) files[prefix + name] = open(path).bufferedReader().use { it.readText() }
            } else {
                walk(path, "$prefix$name/")
            }
        }
    }
    walk(folder, "")
    return files
}

/** Every pack file under the folder the person picked, keyed by its path inside that folder. */
fun Context.pack(folder: Uri): Map<String, String> {
    val root = DocumentFile.fromTreeUri(this, folder) ?: throw IOException("the folder is gone")
    val files = mutableMapOf<String, String>()
    fun walk(dir: DocumentFile, prefix: String) {
        for (file in dir.listFiles()) {
            val name = file.name ?: continue
            if (file.isDirectory) {
                walk(file, "$prefix$name/")
            } else if (isText(name)) {
                val stream = contentResolver.openInputStream(file.uri) ?: continue
                files[prefix + name] = stream.bufferedReader().use { it.readText() }
            }
        }
    }
    walk(root, "")
    return files
}

/** The file at `path` inside the folder the person picked. */
private fun Context.find(folder: Uri, path: String): Uri {
    var file = DocumentFile.fromTreeUri(this, folder)
    for (name in path.split('/')) file = file?.findFile(name)
    return file?.takeIf { it.isFile }?.uri ?: throw IOException("$path is not in the folder")
}

/** The bytes of the file at `path` inside the folder the person picked. */
fun Context.bytes(folder: Uri, path: String): ByteArray {
    val stream = contentResolver.openInputStream(find(folder, path)) ?: throw IOException("$path cannot be read")
    return stream.use { it.readBytes() }
}

/** Writes `text` over the file at `path` inside the folder the person picked. */
fun Context.write(folder: Uri, path: String, text: String) {
    val uri = find(folder, path)
    val stream = contentResolver.openOutputStream(uri, "wt") ?: throw IOException("$path cannot be written")
    stream.bufferedWriter().use { it.write(text) }
}

/**
 * The bundled pack as edited on this device: its files, each replaced by the copy under `dir`
 * when there is one. The assets cannot be written, so edits land in `dir`.
 */
class Overlay(private val dir: File) {
    fun over(files: Map<String, String>): Map<String, String> =
        files + dir.walkTopDown().filter { it.isFile && !it.name.endsWith(NEXT) }.associate {
            it.relativeTo(dir).invariantSeparatorsPath to it.readText()
        }

    fun write(path: String, text: String) {
        val file = File(dir, path)
        if (!file.canonicalPath.startsWith(dir.canonicalPath + File.separator)) {
            throw IOException("$path is not in the pack")
        }
        file.parentFile?.mkdirs()
        file.replace(text.encodeToByteArray())
    }
}

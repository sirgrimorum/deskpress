package dev.deskpress.app

import android.app.Activity
import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.pdf.PdfRenderer
import android.os.ParcelFileDescriptor
import androidx.activity.compose.BackHandler
import androidx.activity.compose.LocalActivity
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color as ComposeColor
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import java.io.Closeable
import java.io.File
import java.io.IOException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * A pack file full screen, each page as wide as the screen, at maximum brightness while it shows:
 * it is what somebody at a counter reads or scans. `read` gives its bytes; it never needs the
 * network.
 */
@Composable
fun Document(state: PackState, title: String, read: () -> ByteArray, close: () -> Unit) {
    val tokens = holder(state).second
    val context = LocalContext.current
    LocalActivity.current?.let { Brightest(it) }
    BackHandler(onBack = close)
    CompositionLocalProvider(LocalTokens provides tokens) {
        Scaffold(topBar = { Header(title.uppercase(), close) }, containerColor = tokens.color("paper")) { padding ->
            BoxWithConstraints(Modifier.fillMaxSize().padding(padding)) {
                val width = with(LocalDensity.current) { maxWidth.roundToPx() }
                // Open while it shows; each page is drawn when its row is, and let go when that row is.
                val opened by produceState<Result<Pdf>?>(null) {
                    val pdf = withContext(Dispatchers.IO) { runCatching { Pdf.open(context.cacheDir, read()) } }
                    value = pdf
                    awaitDispose { pdf.getOrNull()?.close() }
                }
                val shown = opened
                when {
                    shown == null -> {}
                    shown.isFailure -> Text(
                        "The file does not open: ${shown.exceptionOrNull()?.message}",
                        Modifier.padding(tokens.size("spacing.margin")),
                        style = style("body"),
                    )
                    else -> {
                        val pdf = shown.getOrThrow()
                        LazyColumn(verticalArrangement = Arrangement.spacedBy(tokens.size("spacing.gap-s"))) {
                            items(pdf.ratios.size) { i ->
                                val page by produceState<ImageBitmap?>(null, pdf, i, width) {
                                    value = withContext(Dispatchers.IO) { pdf.page(i, width) }
                                }
                                // The page's own shape before it is drawn, so the list does not jump.
                                Box(Modifier.fillMaxWidth().aspectRatio(1 / pdf.ratios[i]).background(ComposeColor.White)) {
                                    page?.let { Image(it, "Page ${i + 1} of ${pdf.ratios.size}", Modifier.fillMaxWidth(), contentScale = ContentScale.FillWidth) }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/** The screen at full brightness until the composable leaves, then as it was. */
@Composable
private fun Brightest(activity: Activity) {
    DisposableEffect(activity) {
        val window = activity.window
        val before = window.attributes.screenBrightness
        window.attributes = window.attributes.apply { screenBrightness = 1f }
        onDispose { window.attributes = window.attributes.apply { screenBrightness = before } }
    }
}

/**
 * A PDF open on the screen, drawn a page at a time. PdfRenderer draws one page at once, so every
 * call holds the lock; a page asked for after `close` is null.
 */
private class Pdf private constructor(private val file: File, private val fd: ParcelFileDescriptor, private val pdf: PdfRenderer) : Closeable {
    /** Each page's height over its width. */
    val ratios: List<Float> = (0 until pdf.pageCount).map { i -> pdf.openPage(i).use { it.height.toFloat() / it.width } }
    private var closed = false

    /** Page `i`, `width` pixels wide on white. */
    fun page(i: Int, width: Int): ImageBitmap? = synchronized(this) {
        if (closed) return null
        pdf.openPage(i).use { page ->
            val bitmap = Bitmap.createBitmap(width, (width * ratios[i]).toInt().coerceAtLeast(1), Bitmap.Config.ARGB_8888)
            bitmap.eraseColor(Color.WHITE)
            page.render(bitmap, null, null, PdfRenderer.Page.RENDER_MODE_FOR_DISPLAY)
            bitmap.asImageBitmap()
        }
    }

    override fun close() {
        synchronized(this) {
            if (closed) return
            closed = true
            pdf.close()
            fd.close()
            file.delete()
        }
    }

    companion object {
        /** The PDF in `bytes`. PdfRenderer reads a seekable file, and an asset or a picked file is a stream. */
        fun open(cache: File, bytes: ByteArray): Pdf {
            val file = File.createTempFile("document", ".pdf", cache)
            var fd: ParcelFileDescriptor? = null
            try {
                file.writeBytes(bytes)
                fd = ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY)
                val pdf = Pdf(file, fd, PdfRenderer(fd))
                if (pdf.ratios.isEmpty()) {
                    pdf.close()
                    throw IOException("it has no pages")
                }
                return pdf
            } catch (e: Exception) {
                fd?.close()
                file.delete()
                throw e
            }
        }
    }
}

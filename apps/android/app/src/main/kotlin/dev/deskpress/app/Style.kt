package dev.deskpress.app

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.drawOutline
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.text.ExperimentalTextApi
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import kotlin.math.abs

/** Atkinson Hyperlegible Next is one variable file: each weight is a setting of its axis. */
@OptIn(ExperimentalTextApi::class)
private val TEXT =
    FontFamily(
        (200..800 step 100).map { w ->
            Font(
                R.font.atkinson_hyperlegible_next,
                FontWeight(w),
                variationSettings = FontVariation.Settings(FontVariation.weight(w)),
            )
        }
    )

private val PIXEL = FontFamily(Font(R.font.jersey_10))

/** A step of the type scale in `color`. Numbers line up in columns: every digit is as wide. */
fun Type.style(color: Color) =
    TextStyle(
        color = color,
        fontSize = size.sp,
        lineHeight = line.sp,
        fontWeight = FontWeight(weight.coerceIn(1, 1000)),
        letterSpacing = tracking.em,
        fontFamily = if (pixel) PIXEL else TEXT,
        fontFeatureSettings = "tnum",
    )

/** The type step `name` in the color token `color`. */
@Composable
fun style(name: String, color: String = "ink"): TextStyle {
    val tokens = LocalTokens.current
    return tokens.type(name).style(tokens.color(color))
}

/** A hard shadow: a copy of the shape offset behind it, or, inset, a band along two inner edges. */
fun Modifier.hard(shadow: Hard?, shape: Shape): Modifier =
    if (shadow == null) {
        this
    } else if (shadow.inset) {
        drawBehind {
            val x = shadow.x * density
            val y = shadow.y * density
            val left = if (x < 0) size.width + x else 0f
            val top = if (y < 0) size.height + y else 0f
            drawRect(shadow.color, Offset(left, 0f), Size(abs(x), size.height))
            drawRect(shadow.color, Offset(0f, top), Size(size.width, abs(y)))
        }
    } else {
        drawBehind {
            translate(shadow.x * density, shadow.y * density) {
                drawOutline(shape.createOutline(size, layoutDirection, this), shadow.color)
            }
        }
    }

package dev.deskpress.app

import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import dev.deskpress.engine.Value

/** One step of the type scale. Sizes are the theme's px, drawn as sp. */
data class Type(
    val size: Float,
    val line: Float,
    val weight: Int,
    val tracking: Float = 0f,
    val pixel: Boolean = false,
)

/** A hard shadow, no blur: an offset copy behind the shape, or a band inside it when inset. */
data class Hard(val x: Float, val y: Float, val color: Color, val inset: Boolean)

/**
 * The tokens a renderer draws with, read from a pack's theme file and completed by [BUILT_IN]
 * where the pack says nothing. Components ask for tokens by name and never for a theme.
 */
data class Tokens(
    val colors: Map<String, Color>,
    val type: Map<String, Type>,
    val sizes: Map<String, Float>,
    val shadows: Map<String, Hard>,
    val kid: Boolean = false,
) {
    fun color(name: String): Color = colors[name] ?: Color.Magenta

    /** In kid mode a `px-` step replaces the plain one where the theme has it. */
    fun type(name: String): Type =
        (if (kid) type["px-$name"] else null) ?: type.getValue(name)

    /** A size like `radius.card` or `touch.min`, in dp. */
    fun size(name: String): Dp = sizes.getValue(name).dp

    /** Every type step at `factor` of its size: the shell's text size setting, over the theme. */
    fun scaled(factor: Float): Tokens =
        if (factor == 1f) this
        else copy(type = type.mapValues { (_, t) -> t.copy(size = t.size * factor, line = t.line * factor) })
}

/** The tokens of the pack being drawn, the built-in theme until one is loaded. */
val LocalTokens = staticCompositionLocalOf { BUILT_IN }

/** The sizes a theme file shares across its themes. */
val GROUPS = listOf("spacing", "radius", "border", "touch")

private fun hex(s: String): Color? {
    val digits = s.removePrefix("#")
    val argb = digits.toLongOrNull(16) ?: return null
    return when (digits.length) {
        6 -> Color(0xFF000000 or argb)
        8 -> Color(((argb and 0xFF) shl 24) or (argb shr 8))
        else -> null
    }
}

/** `-0.02em` or `3px` or `5` as a number. */
private fun number(v: Value?): Float? =
    when (v) {
        is Value.Number -> v.value.toFloat()
        is Value.Text -> v.value.trim().removeSuffix("em").removeSuffix("px").toFloatOrNull()
        else -> null
    }

internal fun Value?.field(key: String): Value? =
    (this as? Value.Fields)?.fields?.firstOrNull { it.key == key }?.value

internal fun Value?.entries(): List<Pair<String, Value>> =
    (this as? Value.Fields)?.fields?.map { it.key to it.value }.orEmpty()

/** `[inset] x y blur color`, as CSS writes a box shadow. */
private fun hard(s: String): Hard? {
    val parts = s.trim().split(Regex("\\s+"))
    val inset = parts.firstOrNull() == "inset"
    val rest = if (inset) parts.drop(1) else parts
    if (rest.size != 4) return null
    val x = number(Value.Text(rest[0])) ?: return null
    val y = number(Value.Text(rest[1])) ?: return null
    return Hard(x, y, hex(rest[3]) ?: return null, inset)
}

/**
 * Which theme of the file to draw when the holder asks for `wanted`: that theme in the system's
 * mode, else as named, else the file's default in the system's mode, else the default. Null when
 * the file has none of them.
 */
fun pick(theme: Value, wanted: String, dark: Boolean): String? {
    val themes = theme.field("themes").entries().map { it.first }.toSet()
    val mode = if (dark) "dark" else "light"
    val default = (theme.field("default") as? Value.Text)?.value.orEmpty()
    val stem = { id: String -> id.removeSuffix("-light").removeSuffix("-dark") }
    val asked = if (wanted.isEmpty()) emptyList() else listOf("${stem(wanted)}-$mode", wanted)
    return (asked + listOf("${stem(default)}-$mode", default)).firstOrNull { it in themes }
}

/** The tokens of theme `id` in the file, over the built-in ones. */
fun tokens(theme: Value, id: String?, dark: Boolean, kid: Boolean): Tokens {
    val base = if (dark) BUILT_IN_DARK else BUILT_IN
    val chosen = id?.let { theme.field("themes").field(it) }
    val colors = chosen.field("colors").entries().mapNotNull { (k, v) ->
        (v as? Value.Text)?.value?.let(::hex)?.let { k to it }
    }
    val shadows = chosen.field("shadow").entries().mapNotNull { (k, v) ->
        (v as? Value.Text)?.value?.let(::hard)?.let { k to it }
    }
    val type = theme.field("type").entries().mapNotNull { (k, v) ->
        val size = number(v.field("size")) ?: return@mapNotNull null
        val step = Type(
            size = size,
            line = number(v.field("line"))?.let { if (it < 4) it * size else it } ?: (size * 1.3f),
            weight = number(v.field("weight"))?.toInt() ?: 400,
            tracking = number(v.field("tracking")) ?: 0f,
            pixel = (v.field("family") as? Value.Text)?.value == "pixel",
        )
        k to step
    }
    val sizes = GROUPS.flatMap { group ->
        theme.field(group).entries().mapNotNull { (k, v) -> number(v)?.let { "$group.$k" to it } }
    }
    return Tokens(
        base.colors + colors,
        base.type + type,
        base.sizes + sizes,
        shadows.toMap(),
        kid,
    )
}

/** What a pack with no theme looks like: quiet, and every promised pair above 4.5:1. */
val BUILT_IN =
    Tokens(
        colors = mapOf(
            "paper" to Color(0xFFFFFFFF),
            "ink" to Color(0xFF1B1B1F),
            "ink-muted" to Color(0xFF55555E),
            "card" to Color(0xFFF4F4F7),
            "card-line" to Color(0xFFC6C6CF),
            "line" to Color(0xFFC6C6CF),
            "rule" to Color(0xFFE3E3E9),
            "soft" to Color(0xFFEDEDF2),
            "alert" to Color(0xFFB3261E),
            "action-bg" to Color(0xFF1B1B1F),
            "action-ink" to Color(0xFFFFFFFF),
            "highlight-bg" to Color(0xFFE3E7FB),
            "highlight-ink" to Color(0xFF1B1B1F),
            "highlight-line" to Color(0xFF3A4BA8),
            "highlight-text" to Color(0xFF2E3C94),
            "chip-bg" to Color(0xFF1B1B1F),
            "chip-ink" to Color(0xFFFFFFFF),
            "bar-bg" to Color(0xFF1B1B1F),
            "bar-ink" to Color(0xFFFFFFFF),
            "bar-muted" to Color(0xFFC6C6CF),
            "missing-fill" to Color(0xFFF4F4F7),
            "missing-border" to Color(0xFF85858F),
            "missing-text" to Color(0xFF45454D),
        ),
        type = mapOf(
            "hero" to Type(56f, 60f, 700, -0.02f),
            "hero-m" to Type(42f, 46f, 700, -0.01f),
            "hero-s" to Type(32f, 36f, 700),
            "title" to Type(24f, 30f, 700),
            "value" to Type(20f, 26f, 600),
            "body" to Type(18f, 26f, 400),
            "body-s" to Type(15f, 21f, 400),
            "action" to Type(18f, 24f, 700),
            "moment" to Type(14f, 18f, 700, 0.06f),
            "label" to Type(13f, 17f, 700, 0.06f),
            "next" to Type(16f, 22f, 600),
        ),
        sizes = mapOf(
            "spacing.margin" to 16f,
            "spacing.gap" to 12f,
            "spacing.gap-s" to 8f,
            "spacing.gap-xs" to 4f,
            "spacing.pad-y" to 14f,
            "spacing.pad-x" to 16f,
            "spacing.pad-top" to 12f,
            "spacing.pad-bottom" to 16f,
            "radius.card" to 16f,
            "radius.action" to 16f,
            "radius.control" to 12f,
            "radius.pill" to 999f,
            "radius.chip" to 999f,
            "radius.kid" to 16f,
            "border.base" to 2f,
            "border.kid" to 4f,
            "border.rule" to 1f,
            "border.badge" to 2f,
            "touch.min" to 48f,
            "touch.action-height" to 56f,
            "touch.row-height" to 52f,
            "touch.chip" to 40f,
        ),
        shadows = emptyMap(),
    )

private val BUILT_IN_DARK =
    BUILT_IN.copy(
        colors = BUILT_IN.colors + mapOf(
            "paper" to Color(0xFF121216),
            "ink" to Color(0xFFEDEDF2),
            "ink-muted" to Color(0xFFB4B4BE),
            "card" to Color(0xFF1E1E24),
            "card-line" to Color(0xFF4A4A54),
            "line" to Color(0xFF4A4A54),
            "rule" to Color(0xFF2E2E36),
            "soft" to Color(0xFF26262D),
            "alert" to Color(0xFFFFB4AB),
            "action-bg" to Color(0xFFEDEDF2),
            "action-ink" to Color(0xFF121216),
            "highlight-bg" to Color(0xFF2A3263),
            "highlight-ink" to Color(0xFFEDEDF2),
            "highlight-line" to Color(0xFFB9C3FF),
            "highlight-text" to Color(0xFFB9C3FF),
            "chip-bg" to Color(0xFFEDEDF2),
            "chip-ink" to Color(0xFF121216),
            "bar-bg" to Color(0xFF2A2A31),
            "bar-ink" to Color(0xFFFFFFFF),
            "bar-muted" to Color(0xFFC6C6CF),
            "missing-fill" to Color(0xFF1E1E24),
            "missing-border" to Color(0xFF8A8A94),
            "missing-text" to Color(0xFFC6C6CF),
        )
    )

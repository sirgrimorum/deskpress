package dev.deskpress.app

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import dev.deskpress.engine.Field
import dev.deskpress.engine.Value
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ThemeTest {
    private fun fields(vararg pairs: Pair<String, Value>) =
        Value.Fields(pairs.map { (k, v) -> Field(k, v) })

    private fun text(s: String) = Value.Text(s)

    private fun file(default: String, vararg ids: String) =
        fields(
            "default" to text(default),
            "themes" to fields(*ids.map { it to fields() }.toTypedArray()),
        )

    @Test
    fun aFileWithoutThemesPicksNothing() {
        assertNull(pick(Value.Null, "rita", dark = false))
        assertNull(pick(file("sea"), "rita", dark = true))
    }

    @Test
    fun theHoldersThemeFollowsTheSystemMode() {
        val file = file("ana-light", "ana-light", "ana-dark", "rita-light", "rita-dark")
        assertEquals("rita-dark", pick(file, "rita-light", dark = true))
        assertEquals("rita-light", pick(file, "rita-light", dark = false))
        assertEquals("rita-light", pick(file, "rita", dark = false))
    }

    @Test
    fun aThemeWithoutAModeIsTakenAsItIs() {
        assertEquals("rita", pick(file("ana", "ana", "rita"), "rita", dark = true))
    }

    @Test
    fun withoutTheHoldersThemeTheDefaultInTheModeWins() {
        val file = file("ana-light", "ana-light", "ana-dark")
        assertEquals("ana-dark", pick(file, "", dark = true))
        assertEquals("ana-dark", pick(file, "gone", dark = true))
        assertEquals("ana", pick(file("ana", "ana"), "", dark = true))
    }

    @Test
    fun whatAThemeDoesNotSayComesFromTheBuiltIn() {
        val tokens = tokens(Value.Null, null, dark = false, kid = false)
        assertEquals(BUILT_IN.colors, tokens.colors)
        assertEquals(16.dp, tokens.size("spacing.margin"))
        assertNull(tokens.shadows["kid-action"])
        assertEquals(Color(0xFF121216), tokens(Value.Null, null, dark = true, kid = false).color("paper"))
    }

    @Test
    fun aThemeIsReadInPxEmAndHex() {
        val file =
            fields(
                "themes" to fields(
                    "t" to fields(
                        "colors" to fields("ink" to text("#102030"), "card" to text("nope"), "soft" to text("#10203040")),
                        "shadow" to fields(
                            "kid-action" to text("3px 4px 0 #000000"),
                            "kid-box" to text("inset -2px -3px 0 #FFFFFF"),
                            "bad" to text("3px 0 #000000"),
                            "worse" to text("a b 0 #000000"),
                        ),
                    )
                ),
                "type" to fields(
                    "hero" to fields(
                        "size" to text("64px"),
                        "line" to Value.Number(1.0),
                        "weight" to Value.Number(800.0),
                        "tracking" to text("-0.02em"),
                        "family" to text("text"),
                    ),
                    "px-hero" to fields("size" to Value.Number(40.0), "line" to text("44px"), "family" to text("pixel")),
                    "broken" to fields("line" to Value.Number(1.0)),
                ),
                "spacing" to fields("margin" to text("20px"), "gap" to Value.Bool(true)),
            )
        val tokens = tokens(file, "t", dark = false, kid = false)
        assertEquals(Color(0xFF102030), tokens.color("ink"))
        assertEquals(BUILT_IN.color("card"), tokens.color("card"))
        assertEquals(Color(0x40102030), tokens.color("soft"))
        assertEquals(Color.Magenta, tokens.color("nothing"))
        assertEquals(Hard(3f, 4f, Color.Black, inset = false), tokens.shadows["kid-action"])
        assertEquals(Hard(-2f, -3f, Color.White, inset = true), tokens.shadows["kid-box"])
        assertNull(tokens.shadows["bad"])
        assertNull(tokens.shadows["worse"])
        assertEquals(Type(64f, 64f, 800, -0.02f), tokens.type("hero"))
        assertEquals(BUILT_IN.type("body"), tokens.type("body"))
        assertEquals(20.dp, tokens.size("spacing.margin"))
        assertEquals(12.dp, tokens.size("spacing.gap"))
        // A child holds the phone: the pixel step replaces the plain one where there is one.
        val kid = tokens(file, "t", dark = false, kid = true)
        assertEquals(Type(40f, 44f, 400, pixel = true), kid.type("hero"))
        assertEquals(BUILT_IN.type("body"), kid.type("body"))
    }
}

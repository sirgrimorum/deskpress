package dev.deskpress.app

import dev.deskpress.engine.Field
import dev.deskpress.engine.Tree
import dev.deskpress.engine.Value
import dev.deskpress.engine.View
import dev.deskpress.engine.Watch
import dev.deskpress.engine.load
import java.io.IOException
import java.time.LocalDateTime
import org.junit.Assert.assertEquals
import org.junit.Test

// These run the real engine: the desk build in target/debug, loaded through JNA.
class PackTest {
    private val manifest =
        "pack: {id: t, name: Test, language: en, timezone: UTC, content: content.yaml}\n"

    private fun files(content: String) =
        mapOf("pack.yaml" to manifest, "content.yaml" to content)

    private fun opened(files: () -> Map<String, String>) = attempt {
        load("pack.yaml", files()).use { show(it.screen(world(NOON, emptyMap()))) }
    }

    @Test
    fun aReaderThatFailsIsAFailureNotACrash() {
        assertEquals(PackState.Failed("assets gone"), opened { throw IOException("assets gone") })
    }

    @Test
    fun aMissingFileIsUnreadable() {
        assertEquals(PackState.Failed("pack.yaml: no such file"), opened { emptyMap() })
    }

    @Test
    fun aPackWithErrorsListsThem() {
        val state = opened { files("days: x\n") } as PackState.Failed
        assertEquals("1 errors", state.reason)
        assertEquals(listOf("days"), state.errors.map { it.at })
    }

    @Test
    fun anActionTheScreenDoesNotHaveIsRefused() {
        val state = attempt {
            load("pack.yaml", files("days: []\n")).use {
                show(it.dispatch(world(NOON, emptyMap()), "fly", Value.Null).view)
            }
        }
        assertEquals(PackState.Failed("\"fly\" is not an action of the screen \"outline\""), state)
    }

    @Test
    fun aNewerTreeIsRefusedNotHalfDrawn() {
        val view = View(Tree(TREE_VERSION + 1u, "s", emptyList(), "", false), Watch("", emptyList()), emptyList())
        assertEquals(PackState.Failed("tree version 5, this app draws 4"), show(view))
    }

    @Test
    fun theWorldIsTheMinuteInThePackTimezone() {
        val world = world(LocalDateTime.parse("2026-04-11T09:05:59"), emptyMap())
        assertEquals("2026-04-11T09:05", world.now)
    }

    /** The facts below as JSON. A raw string escapes nothing, so this is the text itself. */
    private val JSON =
        """{"title":"A \"long\" walk\nuphill\u0007","path":"a\\b","blocks":[2,false,null]}"""

    @Test
    fun theFactsAModelSeesAreJson() {
        val day = Value.Fields(
            listOf(
                Field("title", Value.Text("A \"long\" walk\nuphill\u0007")),
                Field("path", Value.Text("a\\b")),
                Field("blocks", Value.Items(listOf(Value.Number(2.0), Value.Bool(false), Value.Null))),
            ),
        )
        assertEquals(JSON, day.json())
        assertEquals("null", (null as Value?).json())
    }

    @Test
    fun aValueShowsAsText() {
        val list = Value.Items(listOf(Value.Number(2.0), Value.Number(2.5), Value.Bool(true)))
        assertEquals("2, 2.5, true", list.text())
        assertEquals("a", Value.Text("a").text())
        assertEquals("", Value.Fields(listOf(Field("k", Value.Null))).text())
        assertEquals("", null.text())
    }

    private companion object {
        val NOON: LocalDateTime = LocalDateTime.parse("2026-04-11T12:00")
    }
}

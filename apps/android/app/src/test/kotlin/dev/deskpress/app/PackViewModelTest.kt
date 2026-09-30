package dev.deskpress.app

import dev.deskpress.engine.Edit
import dev.deskpress.engine.Value
import java.io.File
import java.io.IOException
import java.time.LocalDateTime
import kotlin.time.Duration.Companion.minutes
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

// These run the real engine on the example pack, with a clock that follows the test's time.
@OptIn(ExperimentalCoroutinesApi::class)
class PackViewModelTest {
    @get:Rule val temp = TemporaryFolder()

    private val example =
        File(System.getProperty("deskpress.examples"), "one-day")
            .listFiles()
            .orEmpty()
            .filter { it.isFile }
            .associate { it.name to it.readText() }

    private fun TestScope.model(
        files: () -> Map<String, String>,
        at: String,
        facts: Facts? = null,
        write: ((String, String) -> Unit)? = null,
        fetch: (String) -> Reply = { Reply(0, "offline") },
    ): PackViewModel {
        val start = LocalDateTime.parse(at)
        val now = { _: java.time.ZoneId -> start.plusNanos(testScheduler.currentTime * 1_000_000) }
        val io = StandardTestDispatcher(testScheduler)
        if (write == null) return PackViewModel(files, now, backgroundScope, io, facts, fetch = fetch)
        return PackViewModel(files, now, backgroundScope, io, facts, write, fetch)
    }

    /** What an edit said: null when it was kept. */
    private fun TestScope.edit(model: PackViewModel, path: List<String>, value: String): String? =
        edited(model, path, value).first

    /** What an edit said, and the edited file it offers to save a copy of. */
    private fun TestScope.edited(model: PackViewModel, path: List<String>, value: String): Pair<String?, Edit?> {
        var out: Pair<String?, Edit?> = "not done" to null
        model.edit(path, value) { why, copy -> out = why to copy }
        runCurrent()
        return out
    }

    private val PackViewModel.theme
        get() = (state.value as PackState.Showing).theme

    private fun Value.at(vararg path: String): Value? =
        path.fold<String, Value?>(this) { v, key -> v.field(key) }

    private val PackViewModel.screen
        get() = (state.value as PackState.Showing).view.tree.screen

    /** A pack with one question, and a screen that asks it in the person's own words. */
    private val asking =
        mapOf(
            "pack.yaml" to
                """
                pack: {id: t, name: T, language: en, timezone: UTC, content: c.yaml}
                questions:
                  now: {ask: What now?, answer: "'a walk'", shortcut: true}
                screens:
                  home:
                    actions:
                      ask: [{open: ask}]
                  ask:
                    state: {typed: "", reply: ""}
                    actions:
                      show: [{set: typed, to: ${'$'}arg}]
                      say: [{do: assistant.ask, with: {question: typed, facts: questions, then: "'said'"}}]
                      said: [{set: reply, to: ${'$'}arg}]
                    layout:
                      - Label: {text: "{typed}|{reply}|{can.assistant}"}
                rules:
                  - {screen: home}
                """.trimIndent() + "\n",
            "c.yaml" to "days: []\n",
        )

    @Test
    fun itStartsLoadingThenHoldsWhatTheEngineSaid() = runTest {
        val model = model({ emptyMap() }, "2026-04-11T09:00")
        assertEquals(PackState.Loading, model.state.value)
        runCurrent()
        assertEquals(PackState.Failed("pack.yaml: no such file"), model.state.value)
        // With no pack there is nothing to act on.
        model.act("points", Value.Null)
        runCurrent()
        assertEquals(PackState.Failed("pack.yaml: no such file"), model.state.value)
    }

    @Test
    fun aTimezoneThePhoneDoesNotKnowFailsTheLoad() = runTest {
        val pack = "pack: {id: t, name: T, language: en, timezone: Mars/Base, content: c.yaml}\n"
        val model = model({ mapOf("pack.yaml" to pack, "c.yaml" to "days: []\n") }, "2026-04-11T09:00")
        runCurrent()
        assertEquals(PackState.Failed("the timezone Mars/Base is not one this phone knows"), model.state.value)
    }

    @Test
    fun theScreenChangesWhenTheWatchRunsOut() = runTest {
        val model = model({ example }, "2026-04-11T09:00")
        runCurrent()
        val state = model.state.value as PackState.Showing
        assertEquals(emptyList<Any>(), state.warnings)
        assertEquals("morning", model.screen)
        assertEquals("2026-04-11T10:30", state.view.watch.until)
        advanceTimeBy(89.minutes)
        runCurrent()
        assertEquals("morning", model.screen)
        advanceTimeBy(1.minutes)
        runCurrent()
        assertEquals("moment", model.screen)
    }

    @Test
    fun aTapOpensTheScreenItNamesAndBackReturns() = runTest {
        val model = model({ example }, "2026-04-11T11:30")
        runCurrent()
        model.act("points", Value.Null)
        runCurrent()
        assertEquals("sheet", model.screen)
        model.act("back", Value.Null)
        runCurrent()
        assertEquals("moment", model.screen)
    }

    @Test
    fun whoHoldsThePhoneIsKeptAndTheRulesDecideAgain() = runTest {
        val model = model({ example }, "2026-04-11T11:30")
        runCurrent()
        model.act("relay", Value.Null)
        runCurrent()
        assertEquals("relay", model.screen)
        model.act("hold", Value.Text("tomas"))
        runCurrent()
        assertEquals("suggestion", model.screen)
    }

    @Test
    fun whatAnActionKeptIsThereOnTheNextRun() = runTest {
        val facts = Facts(File(temp.root, "facts"))
        val first = model({ example }, "2026-04-11T11:30", facts)
        runCurrent()
        first.act("relay", Value.Null)
        runCurrent()
        first.act("hold", Value.Text("tomas"))
        runCurrent()
        val again = model({ example }, "2026-04-11T11:30", facts)
        runCurrent()
        assertEquals("suggestion", again.screen)
    }

    @Test
    fun unlockedRunsThenAndRefusedDoesNot() = runTest {
        // At 10:40 tomas holds the phone where it is not safe -> screen is complete.
        val facts = Facts(File(temp.root, "facts"))
        facts.write("one-day", "holder: tomas\nreturns_to: rita\n")
        val model = model({ example }, "2026-04-11T10:40", facts)
        runCurrent()
        assertEquals("complete", model.screen)

        // Refused unlock: act("more_time") requests unlock, unlock fails, screen stays complete.
        model.unlock = { done -> done(false) }
        model.act("more_time", Value.Null)
        runCurrent()
        assertEquals("complete", model.screen)

        // Successful unlock: act("more_time") requests unlock, unlock succeeds, runs "extend" -> screen becomes kid.
        model.unlock = { done -> done(true) }
        model.act("more_time", Value.Null)
        runCurrent()
        assertEquals("kid", model.screen)
    }

    @Test
    fun aDocumentCallsAndOpensThroughTheHost() = runTest {
        val facts = Facts(File(temp.root, "facts"))
        facts.write("one-day", "holder: rita\n")
        val model = model({ example }, "2026-04-11T11:30", facts)
        var called = ""
        var shown = ""
        model.call = { called = it }
        model.show = { file, title -> shown = "$title: $file" }
        runCurrent()
        for (action in listOf("agenda", "documents")) {
            model.act(action, Value.Null)
            runCurrent()
        }
        val row = (model.state.value as PackState.Showing).view.tree.nodes[1].children[0]
        model.act("open", row.props.getValue("value"))
        runCurrent()
        assertEquals("document", model.screen)
        model.act("call", Value.Text("112"))
        model.act("file", Value.Null)
        runCurrent()
        assertEquals("112" to "Travel insurance: files/insurance-rita.pdf", called to shown)
    }

    @Test
    fun aChildOutOfTheSafePlaceIsCalledBackOnceTheDeviceKnowsWhereItIs() = runTest {
        val facts = Facts(File(temp.root, "facts"))
        facts.write("one-day", "holder: tomas\nreturns_to: rita\n")
        val model = model({ example }, "2026-04-11T11:30", facts)
        runCurrent()
        assertEquals("suggestion", model.screen)
        model.moved(38.0, -9.0)
        runCurrent()
        assertEquals("complete", model.screen)
        model.moved(38.7248, -9.1139)
        runCurrent()
        assertEquals("suggestion", model.screen)
        // Moving inside the same regions asks nothing again.
        val before = model.state.value
        model.moved(38.7249, -9.1139)
        runCurrent()
        assertSame(before, model.state.value)
    }

    @Test
    fun aPositionGoesToThenAndTheMapOpensThroughTheHost() = runTest {
        val head = "pack: {id: t, name: T, language: en, timezone: UTC, content: c.yaml}\n"
        val screens = """
            screens:
              a:
                actions:
                  park: [{do: location.get, with: {then: "'parked'"}}]
                  parked: [{store: car, value: ${'$'}arg}]
                  map: [{do: map.open, with: {lat: store.car.lat, lon: store.car.lon, label: "'Car'"}}]
                layout:
                  - Label: {text: "{store.car.lat}"}
            rules:
              - {screen: a}
        """.trimIndent()
        val files = mapOf("pack.yaml" to head + screens + "\n", "c.yaml" to "days: []\n")
        val model = model({ files }, "2026-04-11T11:30")
        var unlocated = 0
        var opened = ""
        model.unlocated = { unlocated++ }
        model.map = { lat, lon, label -> opened = "$label $lat $lon" }
        runCurrent()
        // No position yet, and no car to open.
        for (action in listOf("park", "map")) model.act(action, Value.Null)
        runCurrent()
        assertEquals(1 to "", unlocated to opened)
        model.moved(1.5, 2.0)
        model.act("park", Value.Null)
        runCurrent()
        model.act("map", Value.Null)
        runCurrent()
        assertEquals("Car 1.5 2.0", opened)
    }

    @Test
    fun aQuestionIsOfferedOutsideTheAppAndOpensTheScreenAtIt() = runTest {
        val model = model({ asking }, "2026-04-11T11:30")
        runCurrent()
        val view = { (model.state.value as PackState.Showing).view }
        assertEquals(listOf("now" to "What now?"), view().shortcuts.map { it.id to it.ask })
        // What a launcher shortcut does: open the questions and show that one.
        model.ask("now")
        runCurrent()
        assertEquals(listOf("now||"), view().tree.nodes.map { it.props["text"].text() })
    }

    @Test
    fun theQuestionTypedGoesToThePhonesOwnModelWithOnlyTheFactsThePackChose() = runTest {
        val model = model({ asking }, "2026-04-11T11:30")
        runCurrent()
        val label = { ((model.state.value as PackState.Showing).view.tree.nodes[0].props["text"]).text() }
        // No model on this phone: `can.assistant` is false and asking does nothing.
        model.act("ask", Value.Null)
        model.act("show", Value.Text("is it far?"))
        model.act("say", Value.Null)
        runCurrent()
        assertEquals("is it far?||", label())
        var asked = "" to ""
        model.assistant = { question, facts -> asked = question to facts.json(); "about ten minutes" }
        model.act("say", Value.Null)
        runCurrent()
        assertEquals("is it far?|about ten minutes|true", label())
        assertEquals("is it far?" to """[{"id":"now","ask":"What now?","answer":"a walk","shortcut":true}]""", asked)
    }

    @Test
    fun aDayInAnotherZoneIsTodayByThatZonesClock() = runTest {
        val head = "pack: {id: t, name: T, language: en, timezone: UTC, content: c.yaml}\n"
        val screens = "modules: {timeline: }\nscreens: {there: {}, home: {}}\nrules: [{when: day, screen: there}, {screen: home}]\n"
        val days = "days: [{date: 2026-04-11, title: A, zone: Asia/Tokyo, blocks: [[\"10:00\", \"Walk\", {zone: Not/AZone}]]}]\n"
        val files = mapOf("pack.yaml" to head + screens, "c.yaml" to days)
        // At 20:00 in UTC it is already 05:00 the next day in Tokyo.
        val late = model({ files }, "2026-04-10T20:00")
        runCurrent()
        assertEquals("there", late.screen)
        val early = model({ files }, "2026-04-10T14:00")
        runCurrent()
        assertEquals("home", early.screen)
    }

    @Test
    fun aCalendarSyncGoesToTheHostWithItsScopeAndPlansAgainstWhatWasWritten() = runTest {
        val head = "pack: {id: t, name: T, language: en, timezone: UTC, content: c.yaml}\n"
        val screens = """
            modules: {timeline: }
            screens:
              a:
                actions:
                  day: [{do: calendar.sync, with: {scope: "'2026-04-11'"}}]
                  trip: [calendar.sync]
            rules:
              - {screen: a}
        """.trimIndent()
        val days = "days: [{date: 2026-04-11, title: A, blocks: [[\"10:00\", \"Walk\"]]}]\n"
        val model = model({ mapOf("pack.yaml" to head + screens + "\n", "c.yaml" to days) }, "2026-04-10T09:00")
        assertEquals(null, model.plan("", emptyMap()))
        val scopes = mutableListOf<String>()
        model.sync = { scopes += it }
        runCurrent()
        for (action in listOf("day", "trip")) model.act(action, Value.Null)
        runCurrent()
        assertEquals(listOf("2026-04-11", ""), scopes)
        assertEquals("t", model.packId)
        assertEquals("UTC", model.timezone.id)
        val plan = model.plan("", mapOf("2026-04-11.blocks.7" to "x"))!!
        assertEquals(listOf("Walk"), plan.add.map { it.title })
        assertEquals(listOf("2026-04-11.blocks.7"), plan.remove)
    }

    @Test
    fun aSyncFetchesOnlyWhenAllowedAndDrawsWhatCameBack() = runTest {
        val head = "pack: {id: t, name: T, language: en, timezone: UTC, content: c.yaml}\n"
        val screens = """
            modules:
              climate:
                sync:
                  trigger: auto
                  every: 6h
                  request:
                    url: "https://api.example.org/f"
                    query: {day: now.date}
                    secret: {name: key, param: k}
                  read: {date: d, high: max}
            screens:
              a:
                actions:
                  go: [climate.sync]
                layout:
                  - Label: {text: weather.as_of}
                  - Label: {text: weather.failed}
            rules:
              - {screen: a}
        """.trimIndent()
        val files = mapOf("pack.yaml" to head + screens + "\n", "c.yaml" to "days: [{date: 2026-04-11, title: A}]\n")
        val urls = mutableListOf<String>()
        var status = 200
        val model = model({ files }, "2026-04-11T09:00", fetch = { urls += it; Reply(status, "{\"d\": \"2026-04-11\", \"max\": 21}") })
        val asked = mutableListOf<List<String>>()
        var allowed = false
        var key: String? = "s 1"
        val forgotten = mutableListOf<String>()
        model.allow = { asked += it; allowed }
        model.secret = { _, host -> key.takeIf { host == "api.example.org" } }
        model.forget = { name, host -> forgotten += "$name@$host" }
        val labels = { (model.state.value as PackState.Showing).view.tree.nodes.map { it.props["text"].text() } }
        runCurrent()
        // Due at once, but not allowed: nothing is fetched, and no automatic sync asks again.
        assertEquals(listOf(listOf("api.example.org")), asked)
        assertEquals(emptyList<String>(), urls)
        model.moved(1.0, 2.0)
        runCurrent()
        assertEquals(1, asked.size)
        // A tap asks again, and fetches with the secret in its query.
        allowed = true
        model.act("go", Value.Null)
        runCurrent()
        assertEquals(listOf("https://api.example.org/f?day=2026-04-11&k=s+1"), urls)
        assertEquals(listOf("2026-04-11T09:00", ""), labels())
        // A key the server turns down is forgotten, and the last good answer kept.
        advanceTimeBy(1.minutes)
        status = 401
        model.act("go", Value.Null)
        runCurrent()
        assertEquals(listOf("key@api.example.org"), forgotten)
        assertEquals(listOf("2026-04-11T09:00", "the server answered 401"), labels())
        status = 403
        model.act("go", Value.Null)
        runCurrent()
        assertEquals(listOf("key@api.example.org", "key@api.example.org"), forgotten)
        key = null
        model.act("go", Value.Null)
        runCurrent()
        assertEquals(3, urls.size)
        assertEquals(listOf("2026-04-11T09:00", "no key to ask with"), labels())
        // A second tap while the question is open neither asks again nor fetches twice.
        val answer = CompletableDeferred<Boolean>()
        model.allow = { asked += it; answer.await() }
        key = "s 1"
        status = 200
        val before = asked.size
        model.act("go", Value.Null)
        model.act("go", Value.Null)
        runCurrent()
        assertEquals(before + 1, asked.size)
        answer.complete(true)
        runCurrent()
        assertEquals(4, urls.size)
    }

    @Test
    fun anEditThatCannotBeKeptSaysWhyAndChangesNothing() = runTest {
        val model = model({ example }, "2026-04-11T11:30")
        runCurrent()
        val paper = listOf("themes", "plain-light", "colors", "paper")
        assertEquals("not written: this pack is read only", edit(model, paper, "#FAFAFA"))
        val ink = listOf("themes", "plain-light", "colors", "ink")
        val why = edit(model, ink, "#FFFFFF").orEmpty()
        assertTrue(why, why.startsWith("ink on paper is 1.00:1"))
        val nope = listOf("themes", "nope", "colors", "ink")
        assertEquals("theme.yaml: themes.nope.colors.ink: no \"nope\" in a block mapping", edit(model, nope, "#000000"))
        val lost = model({ example }, "2026-04-11T11:30", write = { _, _ -> throw SecurityException() })
        runCurrent()
        val (gone, copy) = edited(lost, paper, "#FAFAFA")
        assertTrue(gone.orEmpty(), gone.orEmpty().startsWith("not written: the folder can no longer be written"))
        // The edit was valid, so the file comes back to be saved elsewhere; a refused one does not.
        assertEquals("theme.yaml", copy?.file)
        assertTrue(copy!!.text.contains("#FAFAFA"))
        assertEquals(null, edited(lost, ink, "#FFFFFF").second)
        assertEquals(Value.Text("#FFFFFF"), model.theme.at("themes", "plain-light", "colors", "paper"))
    }

    @Test
    fun aFileOutsideTheOverlayIsNotWritten() {
        val overlay = Overlay(temp.newFolder("pack"))
        val e = runCatching { overlay.write("../theme.yaml", "") }.exceptionOrNull()
        assertEquals("../theme.yaml is not in the pack", e?.message)
    }

    @Test
    fun aWriteCutShortLeavesTheOldFileAndIsNotReadAsOne() {
        val dir = temp.newFolder("pack")
        val overlay = Overlay(dir)
        overlay.write("theme.yaml", "old")
        File(dir, "theme.yaml$NEXT").writeText("half")
        assertEquals(mapOf("theme.yaml" to "old"), overlay.over(emptyMap()))
        overlay.write("theme.yaml", "new")
        assertEquals(mapOf("theme.yaml" to "new"), overlay.over(emptyMap()))
        // A file that cannot be replaced, here a folder in its place, is an error, not a silent loss.
        File(dir, "sub").mkdirs()
        File(dir, "sub/x").writeText("")
        assertTrue(runCatching { File(dir, "sub").replace(byteArrayOf()) }.exceptionOrNull() is IOException)
    }

    @Test
    fun anEditIsWrittenBackAndDrawnAndSurvivesARestart() = runTest {
        val overlay = Overlay(temp.newFolder("pack"))
        val model = model({ overlay.over(example) }, "2026-04-11T11:30", write = overlay::write)
        runCurrent()
        assertEquals(null, edit(model, listOf("themes", "plain-light", "colors", "paper"), "#FAFAFA"))
        assertEquals(null, edit(model, listOf("spacing", "margin"), "20px"))
        assertEquals(Value.Text("#FAFAFA"), model.theme.at("themes", "plain-light", "colors", "paper"))
        assertEquals("moment", model.screen)
        // The file keeps its comments; only the two values changed.
        val kept = File(temp.root, "pack/theme.yaml").readText()
        val want = example.getValue("theme.yaml")
            .replace("paper: \"#FFFFFF\"", "paper: \"#FAFAFA\"")
            .replace("margin: 16px", "margin: 20px")
        assertEquals(want, kept)
        val again = model({ overlay.over(example) }, "2026-04-11T11:30")
        runCurrent()
        assertEquals(Value.Text("20px"), again.theme.at("spacing", "margin"))
    }
}

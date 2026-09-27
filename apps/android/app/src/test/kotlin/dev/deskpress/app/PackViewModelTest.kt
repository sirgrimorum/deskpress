package dev.deskpress.app

import dev.deskpress.engine.Value
import java.io.File
import java.time.LocalDateTime
import kotlin.time.Duration.Companion.minutes
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Test

// These run the real engine on the example pack, with a clock that follows the test's time.
@OptIn(ExperimentalCoroutinesApi::class)
class PackViewModelTest {
    private val example =
        File(System.getProperty("deskpress.examples"), "one-day")
            .listFiles()
            .orEmpty()
            .associate { it.name to it.readText() }

    private fun TestScope.model(files: () -> Map<String, String>, at: String): PackViewModel {
        val start = LocalDateTime.parse(at)
        val now = { _: java.time.ZoneId -> start.plusNanos(testScheduler.currentTime * 1_000_000) }
        return PackViewModel(files, now, backgroundScope, StandardTestDispatcher(testScheduler))
    }

    private val PackViewModel.screen
        get() = (state.value as PackState.Showing).view.tree.screen

    @Test
    fun itStartsLoadingThenHoldsWhatTheEngineSaid() = runTest {
        val model = model({ emptyMap() }, "2026-04-11T09:00")
        assertEquals(PackState.Loading, model.state.value)
        runCurrent()
        assertEquals(PackState.Failed("pack.yaml: no such file"), model.state.value)
        // With no pack there is nothing to act on.
        model.act("see_place", Value.Null)
        runCurrent()
        assertEquals(PackState.Failed("pack.yaml: no such file"), model.state.value)
    }

    @Test
    fun theScreenChangesWhenTheWatchRunsOut() = runTest {
        val model = model({ example }, "2026-04-11T09:00")
        runCurrent()
        val state = model.state.value as PackState.Showing
        assertEquals(0, state.warnings)
        assertEquals("today", model.screen)
        assertEquals("2026-04-11T10:30", state.view.watch.until)
        advanceTimeBy(89.minutes)
        runCurrent()
        assertEquals("today", model.screen)
        advanceTimeBy(1.minutes)
        runCurrent()
        assertEquals("moment", model.screen)
    }

    @Test
    fun aTapOpensTheScreenItNamesAndBackReturns() = runTest {
        val model = model({ example }, "2026-04-11T11:30")
        runCurrent()
        model.act("see_place", Value.Null)
        runCurrent()
        assertEquals("place", model.screen)
        model.act("back", Value.Null)
        runCurrent()
        assertEquals("moment", model.screen)
    }
}

package dev.deskpress.app

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class PackViewModelTest {
    private val main = StandardTestDispatcher()

    @Before fun setUp() = Dispatchers.setMain(main)

    @After fun tearDown() = Dispatchers.resetMain()

    @Test
    fun itStartsLoadingThenHoldsWhatTheEngineSaid() = runTest(main) {
        val model = PackViewModel({ emptyMap() }, main)
        assertEquals(PackState.Loading, model.state.value)
        advanceUntilIdle()
        assertEquals(PackState.Failed("pack.yaml: no such file"), model.state.value)
    }
}

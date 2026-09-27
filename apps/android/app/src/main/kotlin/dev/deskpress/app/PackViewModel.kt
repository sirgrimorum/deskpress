package dev.deskpress.app

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** Holds the pack across configuration changes. The engine decides; this only carries state. */
class PackViewModel(
    files: () -> Map<String, String>,
    io: CoroutineDispatcher = Dispatchers.IO,
) : ViewModel() {
    private val _state = MutableStateFlow<PackState>(PackState.Loading)
    val state: StateFlow<PackState> = _state.asStateFlow()

    init {
        viewModelScope.launch { _state.value = withContext(io) { open(files) } }
    }
}

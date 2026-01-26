package org.knp.vortex.ui.screens.home

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import org.knp.vortex.data.remote.LibraryDto
import org.knp.vortex.data.remote.MediaItemDto
import org.knp.vortex.data.remote.SeriesDto
import org.knp.vortex.data.repository.HiddenContentRepository
import org.knp.vortex.data.repository.MediaRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

data class HomeUiState(
    val continueWatching: List<MediaItemDto> = emptyList(),
    val recentlyAdded: List<MediaItemDto> = emptyList(),
    val libraries: List<LibraryDto> = emptyList(),
    val visibleLibraries: List<LibraryDto> = emptyList(), // Filtered libraries for display
    val libraryContent: Map<Long, List<MediaItemDto>> = emptyMap(),
    val tvShowLibraryContent: Map<Long, List<SeriesDto>> = emptyMap(),
    val allSeries: List<SeriesDto> = emptyList(),
    val isLoading: Boolean = false,
    val isRefreshing: Boolean = false,
    val error: String? = null,
    val isUnlocked: Boolean = false,
    val isPinSet: Boolean = false,
    val serverUrl: String = ""
)

@HiltViewModel
class HomeViewModel @Inject constructor(
    private val repository: MediaRepository,
    private val hiddenContentRepository: HiddenContentRepository,
    private val settingsRepository: org.knp.vortex.data.repository.SettingsRepository
) : ViewModel() {

    private val _uiState = MutableStateFlow(HomeUiState())
    
    val uiState: StateFlow<HomeUiState> = combine(
        _uiState,
        hiddenContentRepository.isUnlocked
    ) { state, isUnlocked ->
        val visibleLibraries = if (isUnlocked) {
            state.libraries
        } else {
            state.libraries.filter { it.library_type != "other" }
        }
        state.copy(
            visibleLibraries = visibleLibraries,
            isUnlocked = isUnlocked,
            isPinSet = hiddenContentRepository.isPinSet(),
            serverUrl = settingsRepository.getServerUrl()
        )
    }.stateIn(
        viewModelScope,
        SharingStarted.WhileSubscribed(5000),
        HomeUiState()
    )

    init {
        loadData(false)
    }

    fun loadData(isRefresh: Boolean = false) {
        viewModelScope.launch {
            if (isRefresh) {
                _uiState.value = _uiState.value.copy(isRefreshing = true)
            } else {
                _uiState.value = _uiState.value.copy(isLoading = true)
            }
            
            val recentResult = repository.getRecentlyAdded()
            val librariesResult = repository.getLibraries()
            val continueResult = repository.getContinueWatching()
            val seriesResult = repository.getSeries()
            
            val libraries = librariesResult.getOrDefault(emptyList())
            val allSeries = seriesResult.getOrDefault(emptyList())
            
            // Fetch content for each library (max 10 items each)
            val libraryContent = mutableMapOf<Long, List<MediaItemDto>>()
            val tvShowLibraryContent = mutableMapOf<Long, List<SeriesDto>>()
            
            libraries.forEach { lib ->
                if (lib.library_type == "tv_shows") {
                    // For TV Shows, use series data
                    tvShowLibraryContent[lib.id] = allSeries.take(10)
                } else {
                    // For Movies and other types, use library media
                    repository.getLibraryMedia(lib.id).onSuccess { media ->
                        libraryContent[lib.id] = media.take(10)
                    }
                }
            }

            _uiState.value = _uiState.value.copy(
                recentlyAdded = recentResult.getOrDefault(emptyList()),
                libraries = libraries,
                libraryContent = libraryContent,
                tvShowLibraryContent = tvShowLibraryContent,
                continueWatching = continueResult.getOrDefault(emptyList()),
                allSeries = allSeries,
                isLoading = false,
                isRefreshing = false,
                error = if (recentResult.isFailure && !isRefresh) "Failed to connect to server" else null
            )
        }
    }

    fun isPinSet(): Boolean = hiddenContentRepository.isPinSet()

    fun setPin(pin: String) {
        hiddenContentRepository.setPin(pin)
    }

    fun verifyAndUnlock(pin: String): Boolean {
        return if (hiddenContentRepository.verifyPin(pin)) {
            hiddenContentRepository.unlock()
            true
        } else {
            false
        }
    }

    fun lock() {
        hiddenContentRepository.lock()
    }
}

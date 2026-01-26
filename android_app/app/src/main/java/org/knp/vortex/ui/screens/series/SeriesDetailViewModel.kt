package org.knp.vortex.ui.screens.series

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import org.knp.vortex.data.remote.EpisodeDto
import org.knp.vortex.data.remote.SeriesDetailDto
import org.knp.vortex.data.repository.MediaRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject
import java.net.URLDecoder
import java.nio.charset.StandardCharsets

data class SeriesDetailUiState(
    val seriesDetail: SeriesDetailDto? = null,
    val selectedSeason: Int = 1,
    val episodes: List<EpisodeDto> = emptyList(),
    val isLoading: Boolean = false,
    val error: String? = null,
    val serverUrl: String = ""
)

@HiltViewModel
class SeriesDetailViewModel @Inject constructor(
    private val repository: MediaRepository,
    private val settingsRepository: org.knp.vortex.data.repository.SettingsRepository,
    savedStateHandle: SavedStateHandle
) : ViewModel() {

    private val _uiState = MutableStateFlow(SeriesDetailUiState(serverUrl = settingsRepository.getServerUrl()))
    val uiState: StateFlow<SeriesDetailUiState> = _uiState.asStateFlow()

    private val rawSeriesName: String = savedStateHandle.get<String>("seriesName") ?: ""
    val seriesName: String = try {
        URLDecoder.decode(rawSeriesName, StandardCharsets.UTF_8.toString())
    } catch (e: Exception) {
        rawSeriesName
    }

    init {
        loadSeriesDetail()
    }

    fun loadSeriesDetail() {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isLoading = true)
            repository.getSeriesDetail(seriesName)
                .onSuccess { detail ->
                    // Auto-select first season if available
                    val initialSeason = detail.seasons.firstOrNull()?.season_number ?: 1
                    _uiState.value = SeriesDetailUiState(
                        seriesDetail = detail, 
                        selectedSeason = initialSeason,
                        isLoading = false,
                        serverUrl = settingsRepository.getServerUrl()
                    )
                    // Fetch episodes for initial season
                    loadEpisodes(initialSeason)
                }
                .onFailure {
                    _uiState.value = SeriesDetailUiState(isLoading = false, error = it.message)
                }
        }
    }
    
    fun selectSeason(seasonNumber: Int) {
        if (_uiState.value.selectedSeason == seasonNumber && _uiState.value.episodes.isNotEmpty()) return
        
        _uiState.value = _uiState.value.copy(selectedSeason = seasonNumber)
        loadEpisodes(seasonNumber)
    }
    
    private fun loadEpisodes(seasonNumber: Int) {
         viewModelScope.launch {
            // Keep existing data while loading new episodes, just maybe show a small loader or nothing (smooth transition)
             repository.getSeasonEpisodes(seriesName, seasonNumber)
                .onSuccess { episodes ->
                    _uiState.value = _uiState.value.copy(episodes = episodes)
                }
                .onFailure {
                     // Handle error silently or show message type
                }
         }
    }

    fun refreshMetadata() {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isLoading = true)
            repository.refreshSeriesMetadata(seriesName)
                .onSuccess { detail ->
                    _uiState.value = _uiState.value.copy(seriesDetail = detail, isLoading = false)
                }
                .onFailure {
                    _uiState.value = _uiState.value.copy(isLoading = false, error = it.message)
                }
        }
    }
}

package org.knp.vortex.ui.screens.search

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.*
import kotlinx.coroutines.launch
import org.knp.vortex.data.remote.MediaItemDto
import org.knp.vortex.data.remote.SeriesDto
import org.knp.vortex.data.repository.MediaRepository
import javax.inject.Inject

data class SearchUiState(
    val query: String = "",
    val isLoading: Boolean = false,
    val movies: List<MediaItemDto> = emptyList(),
    val series: List<SeriesDto> = emptyList(),
    val error: String? = null
)

@HiltViewModel
class SearchViewModel @Inject constructor(
    private val repository: MediaRepository
) : ViewModel() {

    private val _uiState = MutableStateFlow(SearchUiState())
    val uiState: StateFlow<SearchUiState> = _uiState.asStateFlow()

    // Cache of all media to avoid hitting API on every keystroke
    private var cachedMovies: List<MediaItemDto> = emptyList()
    private var cachedSeries: List<SeriesDto> = emptyList()
    private var isDataLoaded = false

    init {
        // Pre-fetch data or fetch on first search
        loadAllData()
    }

    private fun loadAllData() {
        viewModelScope.launch {
            _uiState.update { it.copy(isLoading = true) }
            
            // Fetch all series
            val seriesResult = repository.getSeries()
            cachedSeries = seriesResult.getOrDefault(emptyList())

            // Fetch all libraries and their movies
            val libResult = repository.getLibraries()
            val libraries = libResult.getOrDefault(emptyList())
            
            val allMovies = mutableListOf<MediaItemDto>()
            libraries.filter { it.library_type == "movies" || it.library_type == "other" }.forEach { lib ->
                repository.getLibraryMedia(lib.id).onSuccess { media ->
                    allMovies.addAll(media)
                }
            }
            cachedMovies = allMovies
            isDataLoaded = true
            
            _uiState.update { it.copy(isLoading = false) }
        }
    }

    fun onQueryChange(query: String) {
        _uiState.update { it.copy(query = query) }
        performSearch(query)
    }

    private fun performSearch(query: String) {
        if (query.isBlank()) {
            _uiState.update { it.copy(movies = emptyList(), series = emptyList()) }
            return
        }

        val lowercaseQuery = query.lowercase()
        
        val filteredMovies = cachedMovies.filter { 
            (it.title?.lowercase()?.contains(lowercaseQuery) == true)
        }
        
        val filteredSeries = cachedSeries.filter {
            (it.name.lowercase().contains(lowercaseQuery))
        }

        _uiState.update { 
            it.copy(movies = filteredMovies, series = filteredSeries) 
        }
    }
}

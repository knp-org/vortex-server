package org.knp.vortex.ui.screens.identify

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import org.knp.vortex.data.remote.MetadataSearchResultDto
import org.knp.vortex.data.repository.MediaRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

data class IdentifyUiState(
    val searchQuery: String = "",
    val searchResults: List<MetadataSearchResultDto> = emptyList(),
    val isLoading: Boolean = false,
    val isIdentifying: Boolean = false,
    val identifySuccess: Boolean = false,
    val error: String? = null
)

@HiltViewModel
class IdentifyViewModel @Inject constructor(
    private val repository: MediaRepository
) : ViewModel() {

    private val _uiState = MutableStateFlow(IdentifyUiState())
    val uiState: StateFlow<IdentifyUiState> = _uiState.asStateFlow()

    fun updateQuery(query: String) {
        _uiState.value = _uiState.value.copy(searchQuery = query)
    }

    fun search(mediaType: String? = null) {
        val query = _uiState.value.searchQuery
        if (query.isBlank()) return

        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isLoading = true, error = null)
            repository.searchMetadata(query, mediaType)
                .onSuccess { results ->
                    _uiState.value = _uiState.value.copy(searchResults = results, isLoading = false)
                }
                .onFailure {
                    _uiState.value = _uiState.value.copy(isLoading = false, error = it.message)
                }
        }
    }

    fun identify(localMediaId: Long, providerId: String, mediaType: String?, seriesName: String? = null) {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isIdentifying = true)
            
            val result = if (localMediaId == 0L && !seriesName.isNullOrEmpty()) {
                repository.identifySeries(seriesName, providerId, mediaType)
            } else {
                repository.identifyMedia(localMediaId, providerId, mediaType)
            }

            result.onSuccess {
                    _uiState.value = _uiState.value.copy(isIdentifying = false, identifySuccess = true)
                }
                .onFailure {
                    _uiState.value = _uiState.value.copy(isIdentifying = false, error = it.message)
                }
        }
    }
}

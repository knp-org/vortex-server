package org.knp.vortex.ui.screens.library

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import org.knp.vortex.data.remote.LibraryDto
import org.knp.vortex.data.repository.MediaRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

data class ManageLibrariesUiState(
    val libraries: List<LibraryDto> = emptyList(),
    val isScanning: Boolean = false,
    val isLoading: Boolean = false,
    val error: String? = null
)

@HiltViewModel
class ManageLibrariesViewModel @Inject constructor(
    private val mediaRepository: MediaRepository
) : ViewModel() {

    private val _uiState = MutableStateFlow(ManageLibrariesUiState())
    val uiState: StateFlow<ManageLibrariesUiState> = _uiState.asStateFlow()

    init {
        loadLibraries()
    }

    fun loadLibraries() {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isLoading = true)
            mediaRepository.getLibraries().onSuccess { libs ->
                _uiState.value = _uiState.value.copy(libraries = libs, isLoading = false)
            }.onFailure { e ->
                _uiState.value = _uiState.value.copy(error = e.message, isLoading = false)
            }
        }
    }

    fun deleteLibrary(id: Long) {
        viewModelScope.launch {
            mediaRepository.deleteLibrary(id)
                .onSuccess {
                    loadLibraries()
                }
                .onFailure { e ->
                    _uiState.value = _uiState.value.copy(error = "Delete failed: ${e.message}")
                }
        }
    }

    fun scanLibraries() {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isScanning = true)
            mediaRepository.scanLibraries().onSuccess {
                _uiState.value = _uiState.value.copy(isScanning = false)
            }.onFailure {
                _uiState.value = _uiState.value.copy(isScanning = false, error = "Scan failed: ${it.message}")
            }
        }
    }
}

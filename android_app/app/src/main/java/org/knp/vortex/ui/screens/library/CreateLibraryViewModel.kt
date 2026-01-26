package org.knp.vortex.ui.screens.library

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import org.knp.vortex.data.remote.DirectoryEntryDto
import org.knp.vortex.data.repository.MediaRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

data class CreateLibraryUiState(
    val name: String = "",
    val path: String = "",
    val type: String = "movies",
    val isLoading: Boolean = false,
    val isSuccess: Boolean = false,
    val error: String? = null,
    val showDirectoryPicker: Boolean = false,
    val directoryPath: String = "",
    val availableDirectories: List<DirectoryEntryDto> = emptyList(),
    val isDirectoryLoading: Boolean = false
)

@HiltViewModel
class CreateLibraryViewModel @Inject constructor(
    private val repository: MediaRepository
) : ViewModel() {

    private val _uiState = MutableStateFlow(CreateLibraryUiState())
    val uiState: StateFlow<CreateLibraryUiState> = _uiState.asStateFlow()

    fun updateName(name: String) {
        _uiState.value = _uiState.value.copy(name = name)
    }

    fun updatePath(path: String) {
        _uiState.value = _uiState.value.copy(path = path)
    }

    fun updateType(type: String) {
        _uiState.value = _uiState.value.copy(type = type)
    }

    fun openDirectoryPicker() {
        val currentPath = _uiState.value.path.ifBlank { "" }
        _uiState.value = _uiState.value.copy(
            showDirectoryPicker = true,
            directoryPath = currentPath
        )
        loadDirectories(if (currentPath.isBlank()) null else currentPath)
    }

    fun closeDirectoryPicker() {
        _uiState.value = _uiState.value.copy(showDirectoryPicker = false)
    }

    fun selectDirectory(path: String) {
        _uiState.value = _uiState.value.copy(
            path = path,
            showDirectoryPicker = false
        )
    }

    fun navigateDirectory(path: String) {
        // Handle ".." logic locally or just rely on backend? 
        // Backend "listDirectories" lists contents.
        // If ".." logic is needed, we usually strip last segment.
        // For now, let's assume path passed is the full path to enter.
        
        var newPath = path
        if (path == "..") {
             // Basic parent logic - naive
             val current = _uiState.value.directoryPath
             // Assuming windows "\\" or unix "/"
             // Better: Let backend give us ".." entry with full path or handle in backend.
             // Since we modified backend to NOT return "..", let's handle it or re-implement backend.
             // Actually, simplest is to just rely on user clicking folders.
             // If user wants up, they hit Back in Dialog which is weird.
             // Let's implement basic ".." support in UI: 
             // BUT simpler: let's just make onNavigate accept the full new path.
             // The DirectoryEntryDto has `path` field which is full path. So we just use that.
             // For "Up", we need to compute parent.
             // Simplistic parent computation:
             val separators = charArrayOf('/', '\\')
             val lastSep = current.lastIndexOfAny(separators)
             if (lastSep > 0) {
                 newPath = current.substring(0, lastSep)
                 // Special case: C:\ becomes C: which works?
                 if (newPath.endsWith(":")) newPath += "\\" 
             } else {
                 newPath = "" // Root
             }
        }
        
        _uiState.value = _uiState.value.copy(directoryPath = newPath)
        loadDirectories(if (newPath.isBlank()) null else newPath)
    }

    private fun loadDirectories(path: String?) {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isDirectoryLoading = true)
            val result = repository.listDirectories(path)
            result.onSuccess { dirs ->
                _uiState.value = _uiState.value.copy(
                    isDirectoryLoading = false,
                    availableDirectories = dirs
                )
            }.onFailure {
                _uiState.value = _uiState.value.copy(
                    isDirectoryLoading = false,
                    // Optionally show error toast
                )
            }
        }
    }

    fun createLibrary() {
        val currentState = _uiState.value
        if (currentState.name.isBlank() || currentState.path.isBlank()) {
            _uiState.value = currentState.copy(error = "Name and Path are required")
            return
        }

        viewModelScope.launch {
            _uiState.value = currentState.copy(isLoading = true, error = null)
            val result = repository.createLibrary(
                currentState.name, 
                currentState.path, 
                currentState.type
            )
            result.onSuccess {
                _uiState.value = currentState.copy(isLoading = false, isSuccess = true)
            }.onFailure {
                _uiState.value = currentState.copy(isLoading = false, error = it.message ?: "Unknown error")
            }
        }
    }

    fun resetState() {
        _uiState.value = CreateLibraryUiState()
    }
}

package org.knp.vortex.ui.screens.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import org.knp.vortex.data.remote.LibraryDto
import org.knp.vortex.data.repository.MediaRepository
import org.knp.vortex.data.repository.SettingsRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

data class SettingsUiState(
    val serverUrl: String = "",
    val tmdbApiKey: String = "",
    val isBiometricEnabled: Boolean = false,
    val isLoading: Boolean = false,
    val isSaved: Boolean = false,
    val error: String? = null
)

@HiltViewModel
class SettingsViewModel @Inject constructor(
    private val settingsRepository: SettingsRepository,
    private val mediaRepository: MediaRepository
) : ViewModel() {

    private val _uiState = MutableStateFlow(SettingsUiState())
    val uiState: StateFlow<SettingsUiState> = _uiState.asStateFlow()

    init {
        loadSettings()
        loadRemoteSettings()
    }

    private fun loadSettings() {
        val currentUrl = settingsRepository.getServerUrl()
        val biometric = settingsRepository.isBiometricEnabled()
        
        _uiState.value = _uiState.value.copy(
            serverUrl = currentUrl,
            isBiometricEnabled = biometric
        )
    }

    private fun loadRemoteSettings() {
        viewModelScope.launch {
            mediaRepository.getSettings().onSuccess { settingsList ->
                val key = settingsList.find { it.key == "tmdb_api_key" }?.value ?: ""
                _uiState.value = _uiState.value.copy(tmdbApiKey = key)
            }
        }
    }
    
    fun updateServerUrl(url: String) {
        _uiState.value = _uiState.value.copy(serverUrl = url, isSaved = false)
    }

    fun updateTmdbApiKey(key: String) {
        _uiState.value = _uiState.value.copy(tmdbApiKey = key, isSaved = false)
    }
    
    fun toggleBiometric(enabled: Boolean) {
        settingsRepository.setBiometricEnabled(enabled)
        _uiState.value = _uiState.value.copy(isBiometricEnabled = enabled)
    }

    fun saveSettings() {
        viewModelScope.launch {
            settingsRepository.setServerUrl(_uiState.value.serverUrl)
            mediaRepository.updateRemoteSetting("tmdb_api_key", _uiState.value.tmdbApiKey)
            _uiState.value = _uiState.value.copy(isSaved = true)
        }
    }
    
    fun resetToDefault() {
        val defaultUrl = settingsRepository.getDefaultUrl()
        _uiState.value = _uiState.value.copy(serverUrl = defaultUrl, isSaved = false)
    }
    
    fun resetDatabase() {
        viewModelScope.launch {
            mediaRepository.resetDatabase()
        }
    }
}

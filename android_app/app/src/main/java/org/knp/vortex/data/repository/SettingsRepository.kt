package org.knp.vortex.data.repository

import android.content.Context
import android.content.SharedPreferences
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class SettingsRepository @Inject constructor(
    @ApplicationContext private val context: Context
) {
    companion object {
        private const val PREFS_NAME = "media_server_settings"
        private const val KEY_SERVER_URL = "server_url"
        private const val KEY_BIOMETRIC_ENABLED = "biometric_enabled"
        private const val DEFAULT_URL = "http://127.0.0.1:3000"
    }

    private val prefs: SharedPreferences = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
    
    private val _serverUrl = MutableStateFlow(getServerUrl())
    val serverUrl: StateFlow<String> = _serverUrl.asStateFlow()

    private val _isBiometricEnabled = MutableStateFlow(isBiometricEnabled())
    val biometricEnabled: StateFlow<Boolean> = _isBiometricEnabled.asStateFlow()

    fun getServerUrl(): String {
        return prefs.getString(KEY_SERVER_URL, DEFAULT_URL) ?: DEFAULT_URL
    }

    fun setServerUrl(url: String) {
        val normalizedUrl = url.trim().removeSuffix("/")
        prefs.edit().putString(KEY_SERVER_URL, normalizedUrl).apply()
        _serverUrl.value = normalizedUrl
    }

    fun isBiometricEnabled(): Boolean {
        return prefs.getBoolean(KEY_BIOMETRIC_ENABLED, false)
    }

    fun setBiometricEnabled(enabled: Boolean) {
        prefs.edit().putBoolean(KEY_BIOMETRIC_ENABLED, enabled).apply()
        _isBiometricEnabled.value = enabled
    }

    fun getDefaultUrl(): String = DEFAULT_URL
}

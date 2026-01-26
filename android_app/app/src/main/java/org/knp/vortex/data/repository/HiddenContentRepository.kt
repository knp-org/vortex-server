package org.knp.vortex.data.repository

import android.content.Context
import android.content.SharedPreferences
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Repository for managing hidden content PIN and unlock state.
 * Uses EncryptedSharedPreferences for secure PIN storage.
 */
@Singleton
class HiddenContentRepository @Inject constructor(
    @ApplicationContext private val context: Context
) {
    companion object {
        private const val PREFS_NAME = "hidden_content_prefs"
        private const val KEY_PIN = "hidden_pin"
    }

    private val masterKey = MasterKey.Builder(context)
        .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
        .build()

    private val encryptedPrefs: SharedPreferences = EncryptedSharedPreferences.create(
        context,
        PREFS_NAME,
        masterKey,
        EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
        EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM
    )

    private val _isUnlocked = MutableStateFlow(false)
    val isUnlocked: StateFlow<Boolean> = _isUnlocked.asStateFlow()

    /**
     * Check if a PIN has been set
     */
    fun isPinSet(): Boolean {
        return encryptedPrefs.getString(KEY_PIN, null) != null
    }

    /**
     * Set a new PIN
     */
    fun setPin(pin: String) {
        encryptedPrefs.edit().putString(KEY_PIN, pin).apply()
    }

    /**
     * Verify if the provided PIN matches the stored PIN
     */
    fun verifyPin(pin: String): Boolean {
        val storedPin = encryptedPrefs.getString(KEY_PIN, null)
        return storedPin != null && storedPin == pin
    }

    /**
     * Unlock hidden content (call after successful PIN verification)
     */
    fun unlock() {
        _isUnlocked.value = true
    }

    /**
     * Lock hidden content
     */
    fun lock() {
        _isUnlocked.value = false
    }

    /**
     * Clear the stored PIN
     */
    fun clearPin() {
        encryptedPrefs.edit().remove(KEY_PIN).apply()
        _isUnlocked.value = false
    }
}

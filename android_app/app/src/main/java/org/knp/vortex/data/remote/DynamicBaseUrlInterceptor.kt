package org.knp.vortex.data.remote

import org.knp.vortex.data.repository.SettingsRepository
import okhttp3.HttpUrl.Companion.toHttpUrlOrNull
import okhttp3.Interceptor
import okhttp3.Response
import javax.inject.Inject
import javax.inject.Singleton

/**
 * OkHttp Interceptor that dynamically replaces the base URL on each request.
 * This allows the server URL to be changed at runtime without recreating
 * the Retrofit instance or restarting the app.
 */
@Singleton
class DynamicBaseUrlInterceptor @Inject constructor(
    private val settingsRepository: SettingsRepository
) : Interceptor {

    override fun intercept(chain: Interceptor.Chain): Response {
        val originalRequest = chain.request()
        val originalUrl = originalRequest.url

        // Get the current server URL from settings
        val newBaseUrl = settingsRepository.getServerUrl().toHttpUrlOrNull()
            ?: return chain.proceed(originalRequest) // Fallback to original if URL is invalid

        // Build new URL with the dynamic base but keeping the path and query
        val newUrl = originalUrl.newBuilder()
            .scheme(newBaseUrl.scheme)
            .host(newBaseUrl.host)
            .port(newBaseUrl.port)
            .build()

        val newRequest = originalRequest.newBuilder()
            .url(newUrl)
            .build()

        return chain.proceed(newRequest)
    }
}

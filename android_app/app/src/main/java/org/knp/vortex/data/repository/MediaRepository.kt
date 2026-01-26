package org.knp.vortex.data.repository

import org.knp.vortex.data.remote.MediaApi
import org.knp.vortex.data.remote.MediaItemDto
import org.knp.vortex.data.remote.ProgressDto
import org.knp.vortex.data.remote.CreateLibraryRequest
import org.knp.vortex.data.remote.ListDirectoriesRequest
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class MediaRepository @Inject constructor(
    private val api: MediaApi
) {
    suspend fun getRecentlyAdded(): Result<List<MediaItemDto>> = runCatching {
        api.getRecentlyAdded()
    }

    suspend fun getLibraries() = runCatching { api.getLibraries() }

    suspend fun createLibrary(name: String, path: String, type: String) = runCatching {
        api.createLibrary(CreateLibraryRequest(name, path, type))
    }

    suspend fun listDirectories(path: String?) = runCatching { 
        api.listDirectories(ListDirectoriesRequest(path)) 
    }

    suspend fun scanLibraries() = runCatching { api.scanLibraries() }

    suspend fun deleteLibrary(id: Long) = runCatching { 
        val response = api.deleteLibrary(id)
        if (!response.isSuccessful) throw Exception("Delete failed: ${response.code()}")
    }

    suspend fun getLibraryMedia(id: Long) = runCatching { api.getLibraryMedia(id) }

    suspend fun browseLibrary(id: Long, path: String?) = runCatching { api.browseLibrary(id, path) }

    suspend fun getContinueWatching() = runCatching { api.getContinueWatching() }

    suspend fun getProgress(id: Long) = runCatching { api.getProgress(id) }

    suspend fun updateProgress(id: Long, position: Long, total: Long) = runCatching {
        api.updateProgress(id, ProgressDto(position, total))
    }

    suspend fun getMediaDetails(id: Long) = runCatching { api.getMediaDetails(id) }

    suspend fun refreshMetadata(id: Long) = runCatching { api.refreshMetadata(id) }

    suspend fun searchMetadata(query: String, mediaType: String?) = runCatching { api.searchMetadata(query, mediaType) }
    
    suspend fun searchLibrary(query: String, mediaType: String?) = runCatching { api.searchLibrary(query, mediaType) }

    suspend fun identifyMedia(id: Long, providerId: String, mediaType: String?) = runCatching {
        api.identifyMedia(id, org.knp.vortex.data.remote.IdentifyRequest(providerId, mediaType))
    }

    // TV Show methods
    suspend fun getSeries() = runCatching { api.getSeries() }
    
    suspend fun getSeriesSeasons(name: String) = runCatching { api.getSeriesSeasons(name) }
    
    suspend fun getSeasonEpisodes(name: String, num: Int) = runCatching { 
        api.getSeasonEpisodes(name, num) 
    }

    suspend fun getSeriesDetail(name: String) = runCatching { api.getSeriesDetail(name) }

    suspend fun refreshSeriesMetadata(name: String) = runCatching { api.refreshSeriesMetadata(name) }

    suspend fun identifySeries(name: String, providerId: String, mediaType: String?) = runCatching {
        api.identifySeries(name, org.knp.vortex.data.remote.IdentifyRequest(providerId, mediaType))
    }

    suspend fun getSettings() = runCatching { api.getSettings() }

    suspend fun updateRemoteSetting(key: String, value: String) = runCatching { 
        api.updateSetting(org.knp.vortex.data.remote.UpdateSettingRequest(key, value)) 
    }

    suspend fun resetDatabase() = runCatching { api.resetDatabase() }

    suspend fun getSubtitles(id: Long) = runCatching { api.getSubtitles(id) }
}

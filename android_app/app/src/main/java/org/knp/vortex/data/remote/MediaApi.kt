package org.knp.vortex.data.remote

import retrofit2.http.GET
import retrofit2.http.POST
import retrofit2.http.Path
import retrofit2.http.Body
import retrofit2.http.Query

const val API_VERSION = "/api/v1"

interface MediaApi {
    @GET("$API_VERSION/recent")
    suspend fun getRecentlyAdded(): List<MediaItemDto>

    @GET("$API_VERSION/libraries")
    suspend fun getLibraries(): List<LibraryDto>

    @GET("$API_VERSION/libraries/{id}/media")
    suspend fun getLibraryMedia(@Path("id") id: Long): List<MediaItemDto>

    @GET("$API_VERSION/libraries/{id}/browse")
    suspend fun browseLibrary(@Path("id") id: Long, @Query("path") path: String?): List<FileSystemEntryDto>

    @GET("$API_VERSION/continue")
    suspend fun getContinueWatching(): List<MediaItemDto>

    @GET("$API_VERSION/media/{id}/progress")
    suspend fun getProgress(@Path("id") id: Long): ProgressDto

    @GET("$API_VERSION/media/{id}")
    suspend fun getMediaDetails(@Path("id") id: Long): MediaItemDto

    @POST("$API_VERSION/media/{id}/refresh")
    suspend fun refreshMetadata(@Path("id") id: Long): MediaItemDto

    @POST("$API_VERSION/media/{id}/progress")
    suspend fun updateProgress(@Path("id") id: Long, @Body progress: ProgressDto)
    @POST("$API_VERSION/libraries")
    suspend fun createLibrary(@Body request: CreateLibraryRequest)

    @POST("$API_VERSION/directories")
    suspend fun listDirectories(@Body request: ListDirectoriesRequest): List<DirectoryEntryDto>

    @POST("$API_VERSION/scan")
    suspend fun scanLibraries()

    @retrofit2.http.DELETE("$API_VERSION/libraries/{id}")
    suspend fun deleteLibrary(@Path("id") id: Long): retrofit2.Response<Unit>

    // TV Show endpoints
    @GET("$API_VERSION/series")
    suspend fun getSeries(): List<SeriesDto>

    @GET("$API_VERSION/series/{name}/seasons")
    suspend fun getSeriesSeasons(@Path("name") name: String): List<SeasonDto>

    @GET("$API_VERSION/series/{name}/detail")
    suspend fun getSeriesDetail(@Path("name") name: String): SeriesDetailDto

    @POST("$API_VERSION/series/{name}/refresh")
    suspend fun refreshSeriesMetadata(@Path("name") name: String): SeriesDetailDto

    @GET("$API_VERSION/series/{name}/season/{num}")
    suspend fun getSeasonEpisodes(@Path("name") name: String, @Path("num") num: Int): List<EpisodeDto>

    @POST("$API_VERSION/series/{name}/identify")
    suspend fun identifySeries(@Path("name") name: String, @Body request: IdentifyRequest): SeriesDetailDto

    // Settings endpoints
    @GET("$API_VERSION/settings")
    suspend fun getSettings(): List<SettingDto>

    @POST("$API_VERSION/settings")
    suspend fun updateSetting(@Body request: UpdateSettingRequest)

    @POST("$API_VERSION/reset")
    suspend fun resetDatabase()

    @GET("$API_VERSION/metadata/search")
    suspend fun searchMetadata(@Query("query") query: String, @Query("media_type") mediaType: String?): List<MetadataSearchResultDto>

    @GET("$API_VERSION/library/search")
    suspend fun searchLibrary(@Query("query") query: String, @Query("media_type") mediaType: String?): List<MediaItemDto>

    @POST("$API_VERSION/media/{id}/identify")
    suspend fun identifyMedia(@Path("id") id: Long, @Body request: IdentifyRequest): MediaItemDto

    @GET("$API_VERSION/stream/{id}/subtitles")
    suspend fun getSubtitles(@Path("id") id: Long): List<SubtitleTrackDto>
}

data class SubtitleTrackDto(
    val id: String,
    val label: String,
    val language: String,
    val source: String,
    val url: String
)

data class MetadataSearchResultDto(
    val title: String,
    val year: String?, // NormalizedMetadata uses Option<String>
    val poster_url: String?,
    val plot: String?, // Renamed from overview
    val provider_ids: Map<String, Any>?,
    val media_type: String?
)

data class IdentifyRequest(
    val provider_id: String,
    val media_type: String?
)

data class ListDirectoriesRequest(
    val path: String? = null
)

data class DirectoryEntryDto(
    val name: String,
    val path: String
)

data class FileSystemEntryDto(
    val name: String,
    val path: String,
    val is_directory: Boolean,
    val media_id: Long?,
    val poster_url: String?
)

data class CreateLibraryRequest(
    val name: String,
    val path: String,
    val library_type: String
)

data class UpdateSettingRequest(
    val key: String,
    val value: String
)

data class SettingDto(
    val key: String,
    val value: String
)

data class MediaItemDto(
    val id: Long,
    val file_path: String,
    val title: String?,
    val year: Long?,
    val poster_url: String?,
    val plot: String?,
    val media_type: String?,
    val series_name: String?,
    val progress: Long?, // Optional for continue watching
    val runtime: Int?,
    val genres: String?,
    val backdrop_url: String?,
    val library_type: String?
)

data class LibraryDto(
    val id: Long,
    val name: String,
    val path: String,
    val library_type: String
)

data class ProgressDto(
    val position: Long,
    val total_duration: Long? = 0
)

// TV Show DTOs
data class SeriesDto(
    val name: String,
    val poster_url: String?,
    val season_count: Int
)

data class SeasonDto(
    val season_number: Int,
    val episode_count: Int,
    val poster_url: String?
)

data class EpisodeDto(
    val id: Long,
    val title: String?,
    val episode_number: Int,
    val poster_url: String?,
    val file_path: String,
    val plot: String?
)

data class SeriesDetailDto(
    val name: String,
    val poster_url: String?,
    val backdrop_url: String?,
    val plot: String?,
    val year: Long?,
    val genres: String?,
    val seasons: List<SeasonDto>
)

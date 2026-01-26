package org.knp.vortex.ui.screens.library

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.clickable
import androidx.compose.foundation.background
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import org.knp.vortex.data.remote.FileSystemEntryDto
import androidx.activity.compose.BackHandler
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.ui.graphics.asImageBitmap
import kotlinx.coroutines.launch
import coil.compose.AsyncImage
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.graphics.Brush
import org.knp.vortex.data.remote.MediaItemDto
import org.knp.vortex.data.remote.SeriesDto
import org.knp.vortex.data.repository.MediaRepository
import org.knp.vortex.ui.components.AppHeader
import org.knp.vortex.ui.components.ModernMediaCard
import org.knp.vortex.ui.components.SectionHeader
import org.knp.vortex.ui.theme.DeepBackground
import org.knp.vortex.ui.theme.PrimaryBlue
import dagger.hilt.android.lifecycle.HiltViewModel
import javax.inject.Inject
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.launch

data class LibraryUiState(
    val isLoading: Boolean = true,
    val mediaItems: List<MediaItemDto> = emptyList(),
    val seriesList: List<SeriesDto> = emptyList(),
    val fileSystemEntries: List<FileSystemEntryDto> = emptyList(),
    val currentPath: String = "",
    val error: String? = null,
    val serverUrl: String = ""
)

@HiltViewModel
class LibraryViewModel @Inject constructor(
    private val repository: MediaRepository,
    private val settingsRepository: org.knp.vortex.data.repository.SettingsRepository
) : ViewModel() {
    var uiState by mutableStateOf(LibraryUiState())
        private set

    init {
        uiState = uiState.copy(serverUrl = settingsRepository.getServerUrl())
    }

    fun loadLibraryContent(libId: Long, libraryType: String) {
        if (uiState.mediaItems.isNotEmpty() || uiState.seriesList.isNotEmpty() || uiState.fileSystemEntries.isNotEmpty()) return // Already loaded
        
        viewModelScope.launch {
            uiState = uiState.copy(isLoading = true, error = null)
            
            val type = libraryType.lowercase()
            when {
                type == "tv_shows" -> {
                    repository.getSeries().onSuccess { allSeries ->
                        uiState = uiState.copy(isLoading = false, seriesList = allSeries)
                    }.onFailure { error -> uiState = uiState.copy(isLoading = false, error = error.message) }
                }
                type == "other" || type == "music_videos" -> {
                    browse(libId, "")
                }
                else -> {
                    repository.getLibraryMedia(libId).onSuccess { items ->
                        uiState = uiState.copy(isLoading = false, mediaItems = items)
                    }.onFailure { error -> uiState = uiState.copy(isLoading = false, error = error.message) }
                }
            }
        }
    }

    fun browse(libId: Long, path: String) {
        viewModelScope.launch {
            uiState = uiState.copy(isLoading = true, error = null)
            repository.browseLibrary(libId, path).onSuccess { entries ->
                 uiState = uiState.copy(
                     isLoading = false, 
                     fileSystemEntries = entries,
                     currentPath = path
                 )
            }.onFailure { error ->
                 uiState = uiState.copy(isLoading = false, error = error.message)
            }
        }
    }

    fun goUp(libId: Long) {
        val current = uiState.currentPath
        if (current.isEmpty()) return
        
        // Remove last segment
        val parts = current.split("/").filter { it.isNotEmpty() }
        val newPath = if (parts.size <= 1) "" else parts.dropLast(1).joinToString("/")
        browse(libId, newPath)
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LibraryScreen(
    libraryId: Long,
    libraryName: String,
    libraryType: String,
    onPlayMedia: (Long, String?) -> Unit,
    onOpenSeries: (String) -> Unit,
    onBack: () -> Unit,
    viewModel: LibraryViewModel = hiltViewModel()
) {
    val uiState = viewModel.uiState
    val displayTitle = libraryName.ifBlank {
        libraryType.replace("_", " ").split(" ").joinToString(" ") { it.replaceFirstChar { c -> c.uppercase() } }
    }
    
    LaunchedEffect(libraryId, libraryType) {
        viewModel.loadLibraryContent(libraryId, libraryType)
    }

    // Handle Back Press for browsing
    BackHandler(enabled = uiState.currentPath.isNotEmpty()) {
        viewModel.goUp(libraryId)
    }

    // Override generic back actions if searching
    val effectiveOnBack = {
        if (uiState.currentPath.isNotEmpty()) {
            viewModel.goUp(libraryId)
        } else {
            onBack()
        }
    }

    org.knp.vortex.ui.components.GlassyBackground {
        Scaffold(
            containerColor = Color.Transparent,
            topBar = {
                AppHeader(
                    title = displayTitle,
                    onBack = onBack,
                    actions = { }
                )
            }
        ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
        ) {
            // Library Name Header
            Box(modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
                SectionHeader(title = if (uiState.currentPath.isEmpty()) displayTitle else "$displayTitle / ...")
            }
            
            when {
                uiState.isLoading -> {
                    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        CircularProgressIndicator(color = PrimaryBlue)
                    }
                }
                uiState.error != null -> {
                    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        Text(text = uiState.error, color = Color.Red)
                    }
                }
                else -> {
                    LazyVerticalGrid(
                        columns = GridCells.Adaptive(minSize = 120.dp),
                        contentPadding = PaddingValues(16.dp),
                        verticalArrangement = Arrangement.spacedBy(16.dp),
                        horizontalArrangement = Arrangement.spacedBy(16.dp),
                        modifier = Modifier.fillMaxSize()
                    ) {
                        if (libraryType == "tv_shows") {
                            items(uiState.seriesList) { series ->
                                ModernMediaCard(
                                    title = series.name,
                                    posterUrl = series.poster_url,
                                    year = null,
                                    onClick = { onOpenSeries(series.name) },
                                    modifier = Modifier.width(140.dp)
                                )
                            }
                        } else if (libraryType.lowercase().let { it == "other" || it == "music_videos" }) {
                            items(uiState.fileSystemEntries) { entry ->
                                val context = androidx.compose.ui.platform.LocalContext.current
                                // Custom Card for Files/Folders
                                org.knp.vortex.ui.components.GlassyCard(
                                    modifier = Modifier
                                        .width(140.dp)
                                        .aspectRatio(1f), // Square for folders/files
                                    onClick = { 
                                        if (entry.is_directory) {
                                            viewModel.browse(libraryId, entry.path)
                                        } else {
                                            if (entry.media_id != null) {
                                                onPlayMedia(entry.media_id, libraryType)
                                            } else {
                                                android.widget.Toast.makeText(context, "Processing media, please wait...", android.widget.Toast.LENGTH_SHORT).show()
                                            }
                                        }
                                    },
                                    shape = androidx.compose.foundation.shape.RoundedCornerShape(12.dp)
                                ) {
                                    Box(modifier = Modifier.fillMaxSize()) {
                                        val context = androidx.compose.ui.platform.LocalContext.current
                                        val thumbnailRequest = androidx.compose.runtime.remember(entry.poster_url, entry.media_id, uiState.serverUrl) {
                                            if (entry.poster_url == null && entry.media_id == null) return@remember null
                                            
                                            // Use poster_url if available, otherwise use server thumbnail endpoint
                                            val imageUrl = entry.poster_url 
                                                ?: "${uiState.serverUrl.trimEnd('/')}/api/v1/media/${entry.media_id}/thumbnail"
                                            
                                            coil.request.ImageRequest.Builder(context)
                                                .data(imageUrl)
                                                .crossfade(true)
                                                .allowHardware(false)
                                                .size(512)
                                                .build()
                                        }
                                        
                                        var isError by androidx.compose.runtime.remember { androidx.compose.runtime.mutableStateOf(false) }
                                        
                                        if (thumbnailRequest != null && !isError) {
                                            AsyncImage(
                                                model = thumbnailRequest,
                                                contentDescription = entry.name,
                                                modifier = Modifier.fillMaxSize(),
                                                contentScale = ContentScale.Crop,
                                                onError = { isError = true }
                                            )
                                            // Overlay the name at the bottom
                                            Box(
                                                modifier = Modifier
                                                    .fillMaxSize()
                                                    .background(
                                                        Brush.verticalGradient(
                                                            colors = listOf(Color.Transparent, Color.Black.copy(alpha = 0.7f)),
                                                            startY = 100f
                                                        )
                                                    )
                                            )
                                            Text(
                                                text = entry.name,
                                                style = MaterialTheme.typography.labelSmall,
                                                color = Color.White,
                                                maxLines = 1,
                                                overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
                                                modifier = Modifier.align(Alignment.BottomStart).padding(8.dp)
                                            )
                                        } else {
                                            Column(
                                                modifier = Modifier.fillMaxSize().padding(8.dp),
                                                horizontalAlignment = Alignment.CenterHorizontally,
                                                verticalArrangement = Arrangement.Center
                                            ) {
                                                Icon(
                                                    imageVector = if (entry.is_directory) androidx.compose.material.icons.Icons.Filled.Folder else androidx.compose.material.icons.Icons.Filled.PlayArrow,
                                                    contentDescription = null,
                                                    tint = if (entry.is_directory) Color(0xFFFFC107) else Color.White,
                                                    modifier = Modifier.size(48.dp)
                                                )
                                                Spacer(modifier = Modifier.height(8.dp))
                                                Text(
                                                    text = entry.name,
                                                    style = MaterialTheme.typography.bodySmall,
                                                    color = Color.White,
                                                    maxLines = 2,
                                                    overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis
                                                )
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            items(uiState.mediaItems) { item ->
                                ModernMediaCard(
                                    title = item.title,
                                    posterUrl = item.poster_url,
                                    year = item.year,
                                    onClick = { onPlayMedia(item.id, libraryType) },
                                    modifier = Modifier.width(140.dp)
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}
}

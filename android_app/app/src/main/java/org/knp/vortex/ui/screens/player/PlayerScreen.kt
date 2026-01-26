package org.knp.vortex.ui.screens.player

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.unit.dp
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Fullscreen
import androidx.compose.material.icons.filled.FullscreenExit
import androidx.media3.common.MediaItem
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.ui.PlayerView
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.knp.vortex.data.repository.MediaRepository
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import javax.inject.Inject
import androidx.hilt.navigation.compose.hiltViewModel
import org.knp.vortex.utils.findActivity

// Note: In a real app, inject Repo via ViewModel. Using direct logic here for brevity if simple service.
// But better to use ViewModel. 

// Quick ViewModel for Player

import org.knp.vortex.data.repository.SettingsRepository
import okhttp3.Call
import okhttp3.OkHttpClient

@HiltViewModel
class PlayerViewModel @Inject constructor(
    private val repository: MediaRepository,
    private val settingsRepository: SettingsRepository,
    private val okHttpClient: OkHttpClient
) : ViewModel() {
    
    val callFactory: Call.Factory get() = okHttpClient
    
    fun getServerUrl(): String = settingsRepository.getServerUrl()
    fun getProgress(id: Long, onResult: (Long) -> Unit) {
        viewModelScope.launch {
            repository.getProgress(id).onSuccess { 
                onResult(it.position) 
            }.onFailure {
                onResult(0L)
            }
        }
    }

    fun getSubtitles(id: Long, onResult: (List<org.knp.vortex.data.remote.SubtitleTrackDto>) -> Unit) {
        viewModelScope.launch {
            repository.getSubtitles(id).onSuccess { 
                onResult(it) 
            }
        }
    }

    fun saveProgress(id: Long, position: Long, total: Long) {
        viewModelScope.launch {
             // Simple throttling could be added here
            repository.updateProgress(id, position, total)
        }
    }
}

// Need to pass VM via Hilt


@Composable
fun PlayerScreen(
    mediaId: Long,
    onBack: () -> Unit,
    viewModel: PlayerViewModel = hiltViewModel()
) {
    val context = LocalContext.current
    var savedPosition by remember { mutableStateOf(0L) }
    var isReady by remember { mutableStateOf(false) }
    var subtitles by remember { mutableStateOf<List<org.knp.vortex.data.remote.SubtitleTrackDto>>(emptyList()) }

    LaunchedEffect(mediaId) {
        viewModel.getSubtitles(mediaId) { subs ->
            subtitles = subs
        }
        viewModel.getProgress(mediaId) { pos ->
            savedPosition = pos
            isReady = true
        }
    }

    if (!isReady) {
        Box(Modifier.fillMaxSize().background(Color.Black), contentAlignment = Alignment.Center) {
            Text("Loading...", color = Color.White)
        }
        return
    }

    var errorMessage by remember { mutableStateOf<String?>(null) }

    val exoPlayer = remember {
        val dataSourceFactory = androidx.media3.datasource.DefaultDataSource.Factory(
            context,
            androidx.media3.datasource.okhttp.OkHttpDataSource.Factory(viewModel.callFactory)
        )
        
        ExoPlayer.Builder(context)
            .setMediaSourceFactory(
                androidx.media3.exoplayer.source.DefaultMediaSourceFactory(dataSourceFactory)
            )
            .build()
            .apply {
                // Dynamic Media URL from Settings
                val baseUrl = viewModel.getServerUrl().trimEnd('/')
                val mediaUrl = "$baseUrl/api/v1/stream/$mediaId"
                
                val mediaItemBuilder = MediaItem.Builder()
                    .setUri(mediaUrl)
                
                val subtitleConfigs = subtitles.map { sub ->
                    val mimeType = if (sub.url.endsWith(".vtt")) androidx.media3.common.MimeTypes.TEXT_VTT else androidx.media3.common.MimeTypes.APPLICATION_SUBRIP
                    val subUrl = "$baseUrl${sub.url}"
                    MediaItem.SubtitleConfiguration.Builder(android.net.Uri.parse(subUrl))
                        .setMimeType(mimeType)
                        .setLanguage(sub.language)
                        .setLabel(sub.label)
                        .setSelectionFlags(androidx.media3.common.C.SELECTION_FLAG_DEFAULT) 
                        .build()
                }
                
                if (subtitleConfigs.isNotEmpty()) {
                    mediaItemBuilder.setSubtitleConfigurations(subtitleConfigs)
                }

                setMediaItem(mediaItemBuilder.build())
                
                addListener(object : androidx.media3.common.Player.Listener {
                    override fun onPlayerError(error: androidx.media3.common.PlaybackException) {
                        errorMessage = "Error: ${error.message}\nCode: ${error.errorCodeName}"
                    }
                })

                prepare()
                if (savedPosition > 0) seekTo(savedPosition * 1000)
                playWhenReady = true
        }
    }
    
    // Auto-save Progress
    LaunchedEffect(exoPlayer) {
        while(true) {
            delay(5000)
            if (exoPlayer.isPlaying) {
                 // DB expects seconds usually, Exo uses ms
                viewModel.saveProgress(mediaId, exoPlayer.currentPosition / 1000, exoPlayer.duration / 1000)
            }
        }
    }

    // Lifecycle handling & Full Screen Mode
    val lifecycleOwner = androidx.lifecycle.compose.LocalLifecycleOwner.current
    var isFullscreen by androidx.compose.runtime.saveable.rememberSaveable { mutableStateOf(false) } // Default to false (Portrait/Normal)

    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_PAUSE) {
                exoPlayer.pause()
                viewModel.saveProgress(mediaId, exoPlayer.currentPosition / 1000, exoPlayer.duration / 1000)
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose {
            lifecycleOwner.lifecycle.removeObserver(observer)
            exoPlayer.release()
        }
    }
    
    // Handle Fullscreen State Side-effects
    val activity = context.findActivity()
    val window = activity?.window

    DisposableEffect(isFullscreen) {
        if (activity != null && window != null) {
            val controller = androidx.core.view.WindowInsetsControllerCompat(window, window.decorView)
            
            if (isFullscreen) {
                // Enter Landscape & Fullscreen
                activity.requestedOrientation = android.content.pm.ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
                androidx.core.view.WindowCompat.setDecorFitsSystemWindows(window, false)
                controller.hide(androidx.core.view.WindowInsetsCompat.Type.systemBars())
                controller.systemBarsBehavior = androidx.core.view.WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            } else {
                // Exit Landscape & Fullscreen
                activity.requestedOrientation = android.content.pm.ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
                androidx.core.view.WindowCompat.setDecorFitsSystemWindows(window, true)
                controller.show(androidx.core.view.WindowInsetsCompat.Type.systemBars())
            }
        }

        onDispose {
            // Reset on cleanup (back press)
             if (activity != null && window != null) {
                 activity.requestedOrientation = android.content.pm.ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
                 androidx.core.view.WindowCompat.setDecorFitsSystemWindows(window, true)
                 androidx.core.view.WindowInsetsControllerCompat(window, window.decorView).show(androidx.core.view.WindowInsetsCompat.Type.systemBars())
             }
        }
    }

    AndroidView(
        factory = {
            PlayerView(context).apply {
                player = exoPlayer
                // Enable native subtitle button
                setShowSubtitleButton(true)
                // Enable native fullscreen button logic
                setFullscreenButtonClickListener {
                    isFullscreen = !isFullscreen
                }
                // Hide controller timeout to allow back button visibility if custom UI
                controllerShowTimeoutMs = 3000
            }
        },
        modifier = Modifier.fillMaxSize().background(Color.Black)
    )
    
    // UI Overlays
    Box(Modifier.fillMaxSize()) {
        // Top Controls
        androidx.compose.foundation.layout.Row(
            modifier = Modifier
                .align(Alignment.TopStart)
                .padding(16.dp)
                .fillMaxWidth(),
            horizontalArrangement = androidx.compose.foundation.layout.Arrangement.SpaceBetween
        ) {
            androidx.compose.material3.IconButton(
                onClick = { 
                    exoPlayer.pause()
                    onBack() 
                }
            ) {
                 androidx.compose.material3.Icon(
                     imageVector = androidx.compose.material.icons.Icons.AutoMirrored.Filled.ArrowBack,
                     contentDescription = "Back",
                     tint = Color.White
                 )
            }
        }

        if (errorMessage != null) {
            Box(
                modifier = Modifier
                    .align(Alignment.Center)
                    .background(Color.Black.copy(alpha = 0.8f))
                    .padding(16.dp)
            ) {
                Text(
                    text = errorMessage!!,
                    color = Color.Red,
                    style = androidx.compose.material3.MaterialTheme.typography.bodyLarge
                )
            }
        }
        
    }
}

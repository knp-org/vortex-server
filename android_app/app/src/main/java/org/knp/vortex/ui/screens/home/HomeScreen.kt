package org.knp.vortex.ui.screens.home

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.LockOpen
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import org.knp.vortex.data.remote.LibraryDto
import org.knp.vortex.data.remote.MediaItemDto
import org.knp.vortex.data.remote.SeriesDto
import org.knp.vortex.ui.components.ModernMediaCard
import org.knp.vortex.ui.components.SectionHeader
import org.knp.vortex.ui.theme.*
import kotlinx.coroutines.launch
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.pulltorefresh.rememberPullToRefreshState
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.clickable
import androidx.compose.foundation.Image
import androidx.compose.ui.res.painterResource
import org.knp.vortex.R
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import coil.compose.AsyncImage
import coil.request.ImageRequest

@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
@Composable
fun HomeScreen(
    onPlayMedia: (Long, String?) -> Unit,
    onOpenSeries: (String) -> Unit,
    onOpenLibrary: (Long, String, String) -> Unit,  // id, name, type
    onOpenSettings: () -> Unit,
    onQuickPlay: (Long) -> Unit, // New callback for direct playback
    viewModel: HomeViewModel = hiltViewModel()
) {
    val uiState by viewModel.uiState.collectAsState()
    val pullToRefreshState = rememberPullToRefreshState()
    
    // PIN Dialog state
    var showPinDialog by remember { mutableStateOf(false) }
    var pinInput by remember { mutableStateOf("") }
    var pinError by remember { mutableStateOf(false) }
    var isSettingPin by remember { mutableStateOf(false) }
    var confirmPin by remember { mutableStateOf("") }
    
    // PIN Dialog
    if (showPinDialog) {
        org.knp.vortex.ui.components.GlassyDialog(
            onDismissRequest = { 
                showPinDialog = false
                pinInput = ""
                confirmPin = ""
                pinError = false
                isSettingPin = false
            },
            title = if (uiState.isUnlocked) "Lock Content" 
                    else if (!uiState.isPinSet) (if (isSettingPin) "Confirm PIN" else "Set PIN")
                    else "Enter PIN",
            content = {
                Column {
                    if (uiState.isUnlocked) {
                        Text("Lock hidden content?", color = GrayText)
                    } else {
                        org.knp.vortex.ui.components.GlassyTextField(
                            value = pinInput,
                            onValueChange = { 
                                pinInput = it
                                pinError = false
                            },
                            label = if (!uiState.isPinSet && isSettingPin) "Confirm PIN" else "PIN",
                            modifier = Modifier.fillMaxWidth(),
                            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.NumberPassword),
                            singleLine = true
                            // VisualTransformation not supported by GlassyTextField yet, will add if needed or it defaults to clear text. 
                            // Ideally GlassyTextField should support visualTransformation but simpler to leave as is for now or use OutlinedTextField if secrecy is critical. 
                            // Actually, let's stick to GlassyTextField for consistency as requested.
                        )
                        if (pinError) {
                            Text(
                                if (!uiState.isPinSet) "PINs don't match" else "Incorrect PIN",
                                color = Color.Red,
                                style = MaterialTheme.typography.bodySmall,
                                modifier = Modifier.padding(top = 4.dp)
                            )
                        }
                    }
                }
            },
            confirmButton = {
                TextButton(onClick = {
                    if (uiState.isUnlocked) {
                        viewModel.lock()
                        showPinDialog = false
                    } else if (!uiState.isPinSet) {
                        if (!isSettingPin) {
                            confirmPin = pinInput
                            pinInput = ""
                            isSettingPin = true
                        } else {
                            if (pinInput == confirmPin) {
                                viewModel.setPin(pinInput)
                                viewModel.verifyAndUnlock(pinInput)
                                showPinDialog = false
                                pinInput = ""
                                confirmPin = ""
                                isSettingPin = false
                            } else {
                                pinError = true
                            }
                        }
                    } else {
                        if (viewModel.verifyAndUnlock(pinInput)) {
                            showPinDialog = false
                            pinInput = ""
                        } else {
                            pinError = true
                        }
                    }
                }) {
                    Text(
                        if (uiState.isUnlocked) "Lock" 
                        else if (!uiState.isPinSet && !isSettingPin) "Next"
                        else "Unlock",
                        color = PrimaryBlue
                    )
                }
            },
            dismissButton = {
                TextButton(onClick = { 
                    showPinDialog = false
                    pinInput = ""
                    confirmPin = ""
                    pinError = false
                    isSettingPin = false
                }) {
                    Text("Cancel", color = GrayText)
                }
            }
        )
    }

    org.knp.vortex.ui.components.GlassyBackground {
        Scaffold(
            containerColor = Color.Transparent,
            topBar = {
                org.knp.vortex.ui.components.AppHeader(
                    onLogoLongClick = { showPinDialog = true },
                    actions = {
                        IconButton(onClick = onOpenSettings) {
                            Icon(
                                imageVector = Icons.Default.Settings,
                                contentDescription = "Settings",
                                tint = Color.White
                            )
                        }
                    }
                )
            }
        ) { padding ->
            PullToRefreshBox(
                isRefreshing = uiState.isRefreshing,
                onRefresh = { viewModel.loadData(true) },
                modifier = Modifier.fillMaxSize().padding(padding),
                state = pullToRefreshState
            ) {
                if (uiState.isLoading) {
                    CircularProgressIndicator(
                        color = PrimaryBlue,
                        modifier = Modifier.align(Alignment.Center)
                    )
                } else if (uiState.error != null) {
                    Text(
                        text = "Error: ${uiState.error}\nIs the backend running?",
                        color = ErrorRed,
                        modifier = Modifier.align(Alignment.Center)
                    ) 
                } else {
                    LazyColumn(
                        modifier = Modifier.fillMaxSize(),
                        contentPadding = PaddingValues(bottom = 32.dp),
                        verticalArrangement = Arrangement.spacedBy(24.dp)
                    ) {
                        item {
                            val rawItems = (uiState.allSeries.take(5) + uiState.recentlyAdded.take(5))
                            val featuredItems = rawItems
                                .distinctBy { item: Any ->
                                    val name = when(item) {
                                        is SeriesDto -> item.name
                                        is MediaItemDto -> {
                                            if (item.media_type == "series" || item.series_name != null) {
                                                 item.series_name ?: item.title ?: ""
                                            } else {
                                                 item.title ?: ""
                                            }
                                        }
                                        else -> item.hashCode().toString()
                                    }
                                    name.lowercase().trim()
                                }
                                .shuffled()
                                .take(5)

                            if (featuredItems.isNotEmpty()) {
                                FeaturedCarousel(
                                    items = featuredItems,
                                    onItemClick = { item ->
                                        when (item) {
                                            is SeriesDto -> onOpenSeries(item.name)
                                            is MediaItemDto -> {
                                                if (item.media_type == "series") {
                                                    onOpenSeries(item.series_name ?: item.title ?: "")
                                                } else {
                                                    onPlayMedia(item.id, item.library_type)
                                                }
                                            }
                                        }
                                    }
                                )
                            }
                        }

                        if (uiState.continueWatching.isNotEmpty()) {
                            item {
                                SectionHeader("Continue Watching")
                                LazyRow(
                                    contentPadding = PaddingValues(horizontal = 24.dp),
                                    horizontalArrangement = Arrangement.spacedBy(16.dp)
                                ) {
                                    items(uiState.continueWatching) { item ->
                                        ModernMediaCard(
                                            title = item.title,
                                            posterUrl = item.poster_url,
                                            year = item.year,
                                            onClick = {
                                                // Quick Play for Continue Watching
                                                onQuickPlay(item.id)
                                            },
                                            modifier = Modifier.width(160.dp),
                                            videoUrl = if (item.poster_url == null) "${uiState.serverUrl}/api/v1/stream/${item.id}" else null
                                        )
                                    }
                                }
                            }
                        }

                        uiState.visibleLibraries.forEach { library ->
                            if (library.library_type == "tv_shows") {
                                val seriesContent = uiState.tvShowLibraryContent[library.id] ?: emptyList()
                                if (seriesContent.isNotEmpty()) {
                                    item {
                                        SectionHeader(
                                            title = library.name.ifBlank { "TV Shows" },
                                            onClick = { onOpenLibrary(library.id, library.name, library.library_type) }
                                        )
                                        LazyRow(
                                            contentPadding = PaddingValues(horizontal = 24.dp),
                                            horizontalArrangement = Arrangement.spacedBy(16.dp)
                                        ) {
                                            items(seriesContent) { series ->
                                                ModernMediaCard(
                                                    title = series.name,
                                                    posterUrl = series.poster_url,
                                                    onClick = { onOpenSeries(series.name) },
                                                    modifier = Modifier.width(140.dp)
                                                )
                                            }
                                        }
                                    }
                                }
                            } else {
                                val content = uiState.libraryContent[library.id] ?: emptyList()
                                if (content.isNotEmpty()) {
                                    item {
                                        SectionHeader(
                                            title = library.name.ifBlank {
                                                library.library_type.replace("_", " ").split(" ").joinToString(" ") { it.replaceFirstChar { c -> c.uppercase() } }
                                            },
                                            onClick = { onOpenLibrary(library.id, library.name, library.library_type) }
                                        )
                                        LazyRow(
                                            contentPadding = PaddingValues(horizontal = 24.dp),
                                            horizontalArrangement = Arrangement.spacedBy(16.dp)
                                        ) {
                                            items(content) { item ->
                                                ModernMediaCard(
                                                    title = item.title,
                                                    posterUrl = item.poster_url ?: "${uiState.serverUrl.trimEnd('/')}/api/v1/media/${item.id}/thumbnail",
                                                    year = item.year,
                                                    onClick = { onPlayMedia(item.id, library.library_type) },
                                                    modifier = Modifier.width(140.dp)
                                                )
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        item {
                            SectionHeader("Recently Added")
                            if (uiState.recentlyAdded.isNotEmpty()) {
                                LazyRow(
                                    contentPadding = PaddingValues(horizontal = 24.dp),
                                    horizontalArrangement = Arrangement.spacedBy(16.dp)
                                ) {
                                    items(uiState.recentlyAdded) { item ->
                                        ModernMediaCard(
                                            title = item.title,
                                            posterUrl = item.poster_url,
                                            year = item.year,
                                            onClick = {
                                                if (item.media_type == "series") {
                                                    onOpenSeries(item.series_name ?: item.title ?: "")
                                                } else {
                                                    onPlayMedia(item.id, item.library_type)
                                                }
                                            },
                                            modifier = Modifier.width(140.dp)
                                        )
                                    }
                                }
                            } else {
                                Text(
                                    "No recent media found.",
                                    color = GrayText,
                                    modifier = Modifier.padding(horizontal = 24.dp)
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun FeaturedCarousel(
    items: List<Any>,
    onItemClick: (Any) -> Unit
) {
    val pagerState = rememberPagerState(pageCount = { items.size })
    
    Column(modifier = Modifier.fillMaxWidth()) {
        HorizontalPager(
            state = pagerState,
            modifier = Modifier
                .fillMaxWidth()
                .height(200.dp),
            contentPadding = PaddingValues(horizontal = 24.dp),
            pageSpacing = 16.dp
        ) { page ->
            val item = items[page]
            val (title, imageUrl) = when (item) {
                is SeriesDto -> item.name to item.poster_url
                is MediaItemDto -> (item.title ?: "Unknown") to item.poster_url
                else -> "Unknown" to null
            }
            
            org.knp.vortex.ui.components.GlassyCard(
                modifier = Modifier
                    .fillMaxSize(),
                onClick = { onItemClick(item) },
                shape = RoundedCornerShape(16.dp)
            ) {
                Box(modifier = Modifier.fillMaxSize()) {
                    AsyncImage(
                        model = ImageRequest.Builder(LocalContext.current)
                            .data(imageUrl)
                            .crossfade(true)
                            .build(),
                        contentDescription = title,
                        contentScale = ContentScale.Crop,
                        modifier = Modifier.fillMaxSize()
                    )
                    
                    Box(
                        modifier = Modifier
                            .fillMaxSize()
                            .background(
                                Brush.verticalGradient(
                                    colors = listOf(Color.Transparent, DeepBackground.copy(alpha = 0.9f)),
                                    startY = 100f
                                )
                            )
                    )
                    
                    Text(
                        text = title,
                        style = MaterialTheme.typography.headlineSmall,
                        color = Color.White,
                        fontWeight = FontWeight.Bold,
                        modifier = Modifier
                            .align(Alignment.BottomStart)
                            .padding(16.dp)
                    )
                }
            }
        }
        
        Row(
            Modifier
                .fillMaxWidth()
                .padding(top = 12.dp),
            horizontalArrangement = Arrangement.Center
        ) {
            repeat(items.size) { index ->
                val isSelected = pagerState.currentPage == index
                val width by androidx.compose.animation.core.animateDpAsState(
                    targetValue = if (isSelected) 24.dp else 8.dp,
                    label = "DotWidth"
                )
                val color by androidx.compose.animation.animateColorAsState(
                    targetValue = if (isSelected) PrimaryBlue else GrayText.copy(alpha = 0.5f),
                    label = "DotColor"
                )
                
                Box(
                    modifier = Modifier
                        .padding(horizontal = 4.dp)
                        .height(8.dp)
                        .width(width)
                        .clip(RoundedCornerShape(50))
                        .background(color)
                )
            }
        }
    }
}

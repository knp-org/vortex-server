package org.knp.vortex.ui.screens.series

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import coil.compose.AsyncImage
import org.knp.vortex.data.remote.EpisodeDto
import org.knp.vortex.ui.components.GlassyTopBar
import org.knp.vortex.ui.theme.*
import androidx.compose.ui.graphics.asImageBitmap
import kotlinx.coroutines.launch

@Composable
fun SeriesDetailScreen(
    onBack: () -> Unit,
    onIdentify: (String) -> Unit,
    onPlayEpisode: (Long) -> Unit,
    viewModel: SeriesDetailViewModel = hiltViewModel()
) {
    val uiState by viewModel.uiState.collectAsState()
    var showMenu by remember { mutableStateOf(false) }

    org.knp.vortex.ui.components.GlassyBackground {
        Scaffold(containerColor = Color.Transparent) { _ ->
            Box(modifier = Modifier.fillMaxSize()) {
                if (uiState.isLoading && uiState.seriesDetail == null) {
                    CircularProgressIndicator(
                        color = PrimaryBlue,
                        modifier = Modifier.align(Alignment.Center)
                    )
                } else if (uiState.seriesDetail != null) {
                    val detail = uiState.seriesDetail!!
                    
                    LazyColumn(
                        contentPadding = PaddingValues(bottom = 32.dp)
                    ) {
                        item {
                            Box(modifier = Modifier.fillMaxWidth().height(450.dp)) {
                                AsyncImage(
                                    model = detail.backdrop_url ?: detail.poster_url,
                                    contentDescription = "Background",
                                    modifier = Modifier
                                        .fillMaxSize()
                                        .background(SurfaceColor),
                                    contentScale = ContentScale.Crop
                                )
                                
                                Box(
                                    modifier = Modifier
                                        .fillMaxSize()
                                        .background(
                                            Brush.verticalGradient(
                                                colors = listOf(Color.Transparent, DeepBackground),
                                                startY = 0f, 
                                                endY = 1300f
                                            )
                                        )
                                )
                                Box(
                                    modifier = Modifier
                                        .align(Alignment.BottomCenter)
                                        .fillMaxWidth()
                                        .height(150.dp)
                                        .background(
                                            Brush.verticalGradient(
                                                colors = listOf(Color.Transparent, DeepBackground),
                                            )
                                        )
                                )

                                Row(
                                    modifier = Modifier
                                        .align(Alignment.BottomStart)
                                        .padding(horizontal = 24.dp, vertical = 24.dp),
                                    verticalAlignment = Alignment.Bottom
                                ) {
                                    Card(
                                        shape = RoundedCornerShape(12.dp),
                                        elevation = CardDefaults.cardElevation(12.dp),
                                        modifier = Modifier.width(140.dp).aspectRatio(0.67f)
                                    ) {
                                        AsyncImage(
                                            model = detail.poster_url,
                                            contentDescription = detail.name,
                                            modifier = Modifier.fillMaxSize(),
                                            contentScale = ContentScale.Crop
                                        )
                                    }
                                    
                                    Spacer(modifier = Modifier.width(16.dp))
                                    
                                    Column(modifier = Modifier.padding(bottom = 8.dp)) {
                                        Text(
                                            text = detail.name,
                                            style = MaterialTheme.typography.headlineMedium,
                                            fontWeight = FontWeight.Bold,
                                            color = Color.White,
                                            maxLines = 2,
                                            overflow = TextOverflow.Ellipsis
                                        )
                                        Spacer(modifier = Modifier.height(4.dp))
                                        if (detail.year != null && detail.year > 0) {
                                            Text(
                                                text = "${detail.year}",
                                                style = MaterialTheme.typography.titleMedium,
                                                color = GrayText
                                            )
                                        }
                                        
                                        if (!detail.genres.isNullOrEmpty()) {
                                            Spacer(modifier = Modifier.height(8.dp))
                                            Row(
                                                modifier = Modifier.fillMaxWidth(),
                                                horizontalArrangement = Arrangement.spacedBy(8.dp)
                                            ) {
                                                detail.genres.split(", ").take(3).forEach { genre ->
                                                    MetadataChip(text = genre, backgroundColor = PrimaryBlue.copy(alpha = 0.2f))
                                                }
                                            }
                                        }
                                        
                                        Spacer(modifier = Modifier.height(12.dp))
                                        Button(
                                            onClick = { /* TODO */ },
                                            colors = ButtonDefaults.buttonColors(containerColor = PrimaryBlue),
                                            shape = RoundedCornerShape(12.dp)
                                        ) {
                                            Icon(Icons.Filled.PlayArrow, contentDescription = null, modifier = Modifier.size(18.dp))
                                            Spacer(modifier = Modifier.width(8.dp))
                                            Text("Play Now")
                                        }
                                    }
                                }
                            }
                        }

                        if (!detail.plot.isNullOrEmpty()) {
                            item {
                                Text(
                                    text = detail.plot,
                                    style = MaterialTheme.typography.bodyLarge,
                                    color = GrayText,
                                    modifier = Modifier.padding(horizontal = 24.dp),
                                    lineHeight = 24.sp
                                )
                                Spacer(modifier = Modifier.height(24.dp))
                            }
                        }

                        item {
                            Text(
                                text = "Seasons",
                                style = MaterialTheme.typography.titleLarge,
                                fontWeight = FontWeight.Bold,
                                color = Color.White,
                                modifier = Modifier.padding(horizontal = 24.dp)
                            )
                            Spacer(modifier = Modifier.height(16.dp))
                            LazyRow(
                                contentPadding = PaddingValues(horizontal = 24.dp),
                                horizontalArrangement = Arrangement.spacedBy(12.dp)
                            ) {
                                items(detail.seasons) { season ->
                                    FilterChip(
                                        selected = season.season_number == uiState.selectedSeason,
                                        onClick = { viewModel.selectSeason(season.season_number) },
                                        label = { Text("Season ${season.season_number}") },
                                        colors = FilterChipDefaults.filterChipColors(
                                            selectedContainerColor = PrimaryBlue,
                                            selectedLabelColor = Color.White,
                                            containerColor = SurfaceColor,
                                            labelColor = GrayText
                                        ),
                                        border = null
                                    )
                                }
                            }
                            Spacer(modifier = Modifier.height(24.dp))
                        }

                        item {
                            if (uiState.episodes.isNotEmpty()) {
                                uiState.episodes.forEach { episode ->
                                    SleekEpisodeItem(episode, uiState.serverUrl, onClick = { onPlayEpisode(episode.id) })
                                    Spacer(modifier = Modifier.height(16.dp))
                                }
                            } else {
                                Box(modifier = Modifier.fillMaxWidth().height(100.dp), contentAlignment = Alignment.Center) {
                                    if (uiState.isLoading) {
                                        CircularProgressIndicator(color = PrimaryBlue)
                                    } else {
                                        Text("No episodes found", color = GrayText)
                                    }
                                }
                            }
                        }
                    }
                }
                
                org.knp.vortex.ui.components.GlassyTopBar(
                    title = "",
                    onBack = onBack,
                    containerColor = Color.Transparent,
                    actions = {
                        Box {
                            IconButton(onClick = { showMenu = true }) {
                                Icon(
                                    imageVector = Icons.Default.MoreVert,
                                    contentDescription = "More",
                                    tint = Color.White
                                )
                            }
                            DropdownMenu(
                                expanded = showMenu,
                                onDismissRequest = { showMenu = false },
                                modifier = Modifier.background(SurfaceColor)
                            ) {
                                DropdownMenuItem(
                                    text = { Text("Refresh Metadata", color = Color.White) },
                                    onClick = {
                                        viewModel.refreshMetadata()
                                        showMenu = false
                                    }
                                )
                                DropdownMenuItem(
                                    text = { Text("Identify", color = Color.White) },
                                    onClick = {
                                        onIdentify(viewModel.seriesName)
                                        showMenu = false
                                    }
                                )
                            }
                        }
                    }
                )
            }
        }
    }
}

@Composable
fun SleekEpisodeItem(episode: EpisodeDto, serverUrl: String, onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 24.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Box(
            modifier = Modifier
                .width(140.dp)
                .height(80.dp)
                .clip(RoundedCornerShape(12.dp))
                .background(SurfaceColor)
        ) {
            val context = androidx.compose.ui.platform.LocalContext.current
            val request = androidx.compose.runtime.remember(episode.poster_url, serverUrl) {
                coil.request.ImageRequest.Builder(context)
                    .data(episode.poster_url ?: "${serverUrl.trimEnd('/')}/api/v1/media/${episode.id}/thumbnail")
                    .crossfade(true)
                    .allowHardware(false)
                    .size(512)
                    .build()
            }
            var isError by androidx.compose.runtime.remember { androidx.compose.runtime.mutableStateOf(false) }

            if (!isError) {
                AsyncImage(
                    model = request,
                    contentDescription = null,
                    modifier = Modifier.fillMaxSize(),
                    contentScale = ContentScale.Crop,
                    onError = { isError = true }
                )
            } else {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Icon(Icons.Filled.PlayArrow, contentDescription = null, tint = Color.White.copy(alpha = 0.5f))
                }
            }
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .background(Color.Black.copy(alpha = 0.3f)),
                contentAlignment = Alignment.Center
            ) {
                Icon(Icons.Filled.PlayArrow, contentDescription = null, tint = Color.White, modifier = Modifier.size(24.dp))
            }
        }
        
        Spacer(modifier = Modifier.width(16.dp))
        
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = "${episode.episode_number}. ${episode.title ?: "Episode ${episode.episode_number}"}",
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Bold,
                color = Color.White,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis
            )
            Spacer(modifier = Modifier.height(4.dp))
            if (!episode.plot.isNullOrEmpty()) {
                Text(
                    text = episode.plot,
                    style = MaterialTheme.typography.bodySmall,
                    color = GrayText,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis
                )
            }
        }
    }
}

@Composable
fun MetadataChip(text: String, backgroundColor: Color = SurfaceColor.copy(alpha = 0.8f)) {
    Surface(
        color = backgroundColor,
        shape = RoundedCornerShape(6.dp)
    ) {
        Text(
            text = text,
            modifier = Modifier.padding(horizontal = 10.dp, vertical = 4.dp),
            color = Color.White,
            style = MaterialTheme.typography.labelMedium
        )
    }
}

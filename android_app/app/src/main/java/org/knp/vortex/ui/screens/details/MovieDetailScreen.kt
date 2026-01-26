package org.knp.vortex.ui.screens.details

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Star
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.mutableStateOf
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
import org.knp.vortex.ui.components.GlassyTopBar
import org.knp.vortex.ui.theme.DeepBackground
import org.knp.vortex.ui.theme.PrimaryBlue
import org.knp.vortex.ui.theme.SurfaceColor
import org.knp.vortex.ui.theme.GrayText

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MovieDetailScreen(
    mediaId: Long,
    onPlay: (Long) -> Unit,
    onBack: () -> Unit,
    onIdentify: (Long, String?, String?) -> Unit = { _, _, _ -> },
    viewModel: MovieDetailViewModel = hiltViewModel()
) {
    val uiState by viewModel.uiState.collectAsState()
    val scrollState = rememberScrollState()
    var showMenu by remember { mutableStateOf(false) }

    LaunchedEffect(mediaId) {
        viewModel.loadMedia(mediaId)
    }

    org.knp.vortex.ui.components.GlassyBackground {
        Scaffold(containerColor = Color.Transparent) { _ ->
            Box(modifier = Modifier.fillMaxSize()) {
                if (uiState.isLoading) {
                    CircularProgressIndicator(
                        modifier = Modifier.align(Alignment.Center),
                        color = PrimaryBlue
                    )
                } else if (uiState.media != null) {
                    val media = uiState.media!!
                    
                    Column(
                        modifier = Modifier
                            .fillMaxSize()
                            .verticalScroll(scrollState)
                    ) {
                        // Header Section (Backdrop + Poster Overlay)
                        Box(
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(450.dp)
                        ) {
                            // Backdrop
                            AsyncImage(
                                model = media.backdrop_url ?: media.poster_url,
                                contentDescription = "Background",
                                modifier = Modifier
                                    .fillMaxSize()
                                    .background(SurfaceColor),
                                contentScale = ContentScale.Crop
                            )
                            
                            // Gradient Overlay (Bottom Up)
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
                            
                            // Solid fade at bottom to merge
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

                            // Content (Poster + Info)
                            Row(
                                modifier = Modifier
                                    .align(Alignment.BottomStart)
                                    .padding(horizontal = 24.dp, vertical = 24.dp),
                                verticalAlignment = Alignment.Bottom
                            ) {
                                // Poster Card
                                Card(
                                    shape = RoundedCornerShape(12.dp),
                                    elevation = CardDefaults.cardElevation(12.dp),
                                    modifier = Modifier.width(140.dp).aspectRatio(0.67f)
                                ) {
                                    AsyncImage(
                                        model = media.poster_url,
                                        contentDescription = media.title,
                                        modifier = Modifier.fillMaxSize(),
                                        contentScale = ContentScale.Crop
                                    )
                                }
                                
                                Spacer(modifier = Modifier.width(16.dp))
                                
                                Column(
                                    modifier = Modifier
                                        .padding(bottom = 8.dp)
                                        .weight(1f) // Fix: Take remaining space
                                ) {
                                    Text(
                                        text = media.title ?: "Unknown",
                                        style = MaterialTheme.typography.headlineMedium,
                                        fontWeight = FontWeight.Bold,
                                        color = Color.White,
                                        maxLines = 2,
                                        overflow = TextOverflow.Ellipsis
                                    )
                                    
                                    Spacer(modifier = Modifier.height(4.dp))
                                    
                                    // Metadata Row
                                    Row(
                                        verticalAlignment = Alignment.CenterVertically,
                                        horizontalArrangement = Arrangement.spacedBy(10.dp)
                                    ) {
                                        if (media.year != null && media.year > 0) {
                                            Text(
                                                text = "${media.year}",
                                                style = MaterialTheme.typography.titleMedium,
                                                color = GrayText
                                            )
                                        }
                                        if (media.runtime != null && media.runtime > 0) {
                                            MetadataChip(text = formatRuntime(media.runtime))
                                        }
                                    }

                                    if (!media.genres.isNullOrEmpty()) {
                                        Spacer(modifier = Modifier.height(8.dp))
                                        // Fix: FlowRow for wrapping genres
                                        @OptIn(ExperimentalLayoutApi::class)
                                        FlowRow(
                                            modifier = Modifier.fillMaxWidth(),
                                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                                            verticalArrangement = Arrangement.spacedBy(8.dp)
                                        ) {
                                            media.genres.split(", ").take(3).forEach { genre ->
                                                MetadataChip(text = genre, backgroundColor = PrimaryBlue.copy(alpha = 0.2f))
                                            }
                                        }
                                    }
                                    
                                    Spacer(modifier = Modifier.height(12.dp))
                                    
                                    // Play Button
                                    Button(
                                        onClick = { onPlay(mediaId) },
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

                        // Description and Other Details
                        Column(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 24.dp, vertical = 24.dp),
                            verticalArrangement = Arrangement.spacedBy(24.dp)
                        ) {
                            // Synopsis
                            if (!media.plot.isNullOrEmpty()) {
                                Column(
                                    verticalArrangement = Arrangement.spacedBy(8.dp)
                                ) {
                                    Text(
                                        text = "Synopsis",
                                        style = MaterialTheme.typography.titleMedium,
                                        fontWeight = FontWeight.Bold,
                                        color = Color.White
                                    )
                                    Text(
                                        text = media.plot,
                                        style = MaterialTheme.typography.bodyLarge,
                                        color = GrayText,
                                        lineHeight = 24.sp
                                    )
                                }
                            }
                            
                            // File Info Card (Glassy)
                            org.knp.vortex.ui.components.GlassyCard(
                                modifier = Modifier.fillMaxWidth(),
                                shape = RoundedCornerShape(12.dp)
                            ) {
                                Column(
                                    modifier = Modifier.padding(16.dp),
                                    verticalArrangement = Arrangement.spacedBy(8.dp)
                                ) {
                                    Text(
                                        text = "File Information",
                                        style = MaterialTheme.typography.titleSmall,
                                        fontWeight = FontWeight.Bold,
                                        color = Color.White
                                    )
                                    Text(
                                        text = media.file_path.substringAfterLast("/").substringAfterLast("\\"),
                                        style = MaterialTheme.typography.bodySmall,
                                        color = GrayText,
                                        maxLines = 2,
                                        overflow = TextOverflow.Ellipsis
                                    )
                                }
                            }
                            
                            // Bottom spacer for safe area
                            Spacer(modifier = Modifier.height(32.dp))
                        }
                    }
                    
                    // Top Bar Overlay
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
                                        text = { Text("Refresh Metadata", color = Color.White) }, // Fixed Text Color
                                        onClick = {
                                            viewModel.refreshMetadata(mediaId)
                                            showMenu = false
                                        }
                                    )
                                    DropdownMenuItem(
                                        text = { Text("Identify", color = Color.White) }, // Fixed Text Color
                                        onClick = {
                                            showMenu = false
                                            onIdentify(mediaId, uiState.media?.title, uiState.media?.media_type)
                                        }
                                    )
                                }
                            }
                        }
                    )
                } else if (uiState.error != null) {
                    Column(
                        modifier = Modifier.align(Alignment.Center),
                        horizontalAlignment = Alignment.CenterHorizontally
                    ) {
                        Text(
                            text = "Error loading media",
                            color = Color.White,
                            fontWeight = FontWeight.Bold
                        )
                        Spacer(modifier = Modifier.height(8.dp))
                        Text(
                            text = uiState.error ?: "Unknown error",
                            color = GrayText
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun MetadataChip(text: String, backgroundColor: Color = SurfaceColor.copy(alpha = 0.8f)) {
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

private fun formatRuntime(minutes: Int): String {
    val h = minutes / 60
    val m = minutes % 60
    return if (h > 0) "${h}h ${if (m < 10) "0$m" else m}m" else "${m}m"
}

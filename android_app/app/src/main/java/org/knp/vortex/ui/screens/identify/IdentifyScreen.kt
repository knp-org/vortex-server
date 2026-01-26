package org.knp.vortex.ui.screens.identify

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import coil.compose.AsyncImage
import org.knp.vortex.data.remote.MetadataSearchResultDto
import org.knp.vortex.ui.theme.DeepBackground
import org.knp.vortex.ui.theme.PrimaryBlue

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun IdentifyScreen(
    mediaId: Long,
    initialTitle: String,
    mediaType: String?,
    seriesName: String? = null,
    onBack: () -> Unit,
    onIdentified: () -> Unit,
    viewModel: IdentifyViewModel = hiltViewModel()
) {
    val uiState by viewModel.uiState.collectAsState()

    LaunchedEffect(initialTitle) {
        viewModel.updateQuery(initialTitle)
        viewModel.search(mediaType)
    }

    LaunchedEffect(uiState.identifySuccess) {
        if (uiState.identifySuccess) {
            onIdentified()
        }
    }

    org.knp.vortex.ui.components.GlassyBackground {
        Scaffold(
            containerColor = Color.Transparent,
            topBar = {
                org.knp.vortex.ui.components.GlassyTopBar(
                    title = "Identify Media",
                    onBack = onBack
                )
            }
        ) { padding ->
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding)
                    .padding(16.dp)
            ) {
                // Search Bar
                org.knp.vortex.ui.components.GlassyTextField(
                    value = uiState.searchQuery,
                    onValueChange = { viewModel.updateQuery(it) },
                    label = "Search Metadata...",
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                    trailingIcon = {
                        IconButton(onClick = { viewModel.search(mediaType) }) {
                            Icon(Icons.Default.Search, contentDescription = "Search", tint = PrimaryBlue)
                        }
                    }
                )

                Spacer(modifier = Modifier.height(16.dp))

                if (uiState.isLoading || uiState.isIdentifying) {
                    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        CircularProgressIndicator(color = PrimaryBlue)
                    }
                } else if (uiState.searchResults.isEmpty()) {
                    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        Text("No results found", color = Color.Gray)
                    }
                } else {
                    LazyColumn(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                        items(uiState.searchResults) { result ->
                            SearchResultCard(
                                result = result,
                                onClick = { 
                                    // Extract the best provider ID safely
                                    val tmdb = result.provider_ids?.get("tmdb")
                                    val providerId = when (tmdb) {
                                        is Number -> tmdb.toLong().toString() // handle 1234.0 case
                                        is String -> tmdb
                                        else -> result.provider_ids?.entries?.firstOrNull()?.let { entry ->
                                            val value = entry.value
                                            if (value is Number) value.toLong().toString() else value.toString()
                                        }
                                    }
                                    
                                    if (providerId != null) {
                                        viewModel.identify(mediaId, providerId, mediaType, seriesName)
                                    }
                                }
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun SearchResultCard(result: MetadataSearchResultDto, onClick: () -> Unit) {
    org.knp.vortex.ui.components.GlassyCard(
        modifier = Modifier.fillMaxWidth(),
        onClick = onClick,
        shape = RoundedCornerShape(12.dp)
    ) {
        Row(modifier = Modifier.padding(12.dp)) {
            AsyncImage(
                model = result.poster_url,
                contentDescription = result.title,
                modifier = Modifier
                    .width(80.dp)
                    .height(120.dp)
                    .clip(RoundedCornerShape(8.dp)),
                contentScale = ContentScale.Crop
            )
            Spacer(modifier = Modifier.width(12.dp))
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = result.title,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Bold,
                    color = Color.White
                )
                if (!result.year.isNullOrEmpty()) {
                    Text(
                        text = result.year,
                        style = MaterialTheme.typography.bodySmall,
                        color = Color.LightGray
                    )
                }
                Spacer(modifier = Modifier.height(8.dp))
                Text(
                    text = result.plot ?: "No Description",
                    style = MaterialTheme.typography.bodySmall,
                    color = Color.Gray,
                    maxLines = 4,
                    overflow = TextOverflow.Ellipsis
                )
            }
        }
    }
}

package org.knp.vortex.ui.screens.search

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Search
import org.knp.vortex.ui.components.*
import org.knp.vortex.ui.theme.*

@Composable
fun SearchScreen(
    onPlayMedia: (Long, String?) -> Unit,
    onOpenSeries: (String) -> Unit,
    viewModel: SearchViewModel = hiltViewModel()
) {
    val uiState by viewModel.uiState.collectAsState()

    org.knp.vortex.ui.components.GlassyBackground {
        Scaffold(
            containerColor = Color.Transparent,
            topBar = {
                Column(modifier = Modifier.padding(top = 16.dp)) {
                    org.knp.vortex.ui.components.AppHeader(subtitle = "Search")
                    Spacer(modifier = Modifier.height(8.dp))
                    org.knp.vortex.ui.components.GlassyTextField(
                        value = uiState.query,
                        onValueChange = { query -> viewModel.onQueryChange(query) },
                        label = "Search movies and series...",
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 24.dp, vertical = 8.dp),
                        keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(
                            imeAction = androidx.compose.ui.text.input.ImeAction.Search
                        )
                    )
                }
            }
        ) { padding ->
        if (uiState.isLoading) {
            Box(Modifier.fillMaxSize(), contentAlignment = androidx.compose.ui.Alignment.Center) {
                CircularProgressIndicator(color = PrimaryBlue)
            }
        } else {
            LazyColumn(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding),
                contentPadding = PaddingValues(16.dp),
                verticalArrangement = Arrangement.spacedBy(24.dp)
            ) {
                if (uiState.query.isBlank()) {
                    item {
                         Box(Modifier.fillMaxWidth().height(200.dp), contentAlignment = androidx.compose.ui.Alignment.Center) {
                              Column(horizontalAlignment = androidx.compose.ui.Alignment.CenterHorizontally) {
                                  Icon(Icons.Default.Search, contentDescription = null, tint = GrayText, modifier = Modifier.size(64.dp))
                                  Text("Search your library", color = GrayText, style = MaterialTheme.typography.titleMedium)
                              }
                         }
                    }
                } else {
                     if (uiState.series.isNotEmpty()) {
                        item {
                            SectionHeader("Series")
                        }
                        item {
                            LazyRow(
                                horizontalArrangement = Arrangement.spacedBy(16.dp)
                            ) {
                                items(uiState.series) { series ->
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

                    if (uiState.movies.isNotEmpty()) {
                        item {
                            SectionHeader("Movies")
                        }
                        item {
                            LazyRow(
                                horizontalArrangement = Arrangement.spacedBy(16.dp)
                            ) {
                                items(uiState.movies) { movie ->
                                    ModernMediaCard(
                                        title = movie.title,
                                        posterUrl = movie.poster_url,
                                        year = movie.year,
                                        onClick = { onPlayMedia(movie.id, movie.library_type) },
                                        modifier = Modifier.width(140.dp)
                                    )
                                }
                            }
                        }
                    }
                    
                    if (uiState.movies.isEmpty() && uiState.series.isEmpty()) {
                         item {
                              Text("No results found for \"${uiState.query}\"", color = GrayText)
                         }
                    }
                }
            }
        }
    }
}
}

package org.knp.vortex.ui.screens.library

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import org.knp.vortex.ui.components.GlassyTopBar
import org.knp.vortex.ui.theme.*
import org.knp.vortex.ui.components.GlassyBackground
import org.knp.vortex.ui.components.GlassyCard
import org.knp.vortex.ui.components.GlassySurface

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ManageLibrariesScreen(
    onBack: () -> Unit,
    onAddLibrary: () -> Unit,
    viewModel: ManageLibrariesViewModel = hiltViewModel()
) {
    val uiState by viewModel.uiState.collectAsState()

    GlassyBackground {
        Scaffold(
            containerColor = Color.Transparent, // Transparent for GlassyBackground
            topBar = {
                GlassyTopBar(title = "Manage Libraries", onBack = onBack)
            },
            floatingActionButton = {
                ExtendedFloatingActionButton(
                    onClick = onAddLibrary,
                    containerColor = PrimaryBlue,
                    contentColor = Color.White,
                    icon = { Icon(Icons.Default.Add, contentDescription = null) },
                    text = { Text("Add Library") }
                )
            }
        ) { padding ->
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding)
                    .padding(16.dp)
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(20.dp)
            ) {
                // Scan Section
                GlassySurface(
                    shape = RoundedCornerShape(16.dp),
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(16.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Column(modifier = Modifier.weight(1f)) {
                            Text(
                                text = "Scan Libraries",
                                style = MaterialTheme.typography.titleMedium,
                                color = Color.White,
                                fontWeight = FontWeight.Bold
                            )
                            Text(
                                text = "Check folders for new media",
                                style = MaterialTheme.typography.bodySmall,
                                color = GrayText
                            )
                        }
                        
                        Button(
                            onClick = { viewModel.scanLibraries() },
                            colors = ButtonDefaults.buttonColors(containerColor = PrimaryBlue),
                            enabled = !uiState.isScanning,
                            contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
                            shape = RoundedCornerShape(12.dp)
                        ) {
                            if (uiState.isScanning) {
                                CircularProgressIndicator(modifier = Modifier.size(18.dp), color = Color.White, strokeWidth = 2.dp)
                                Spacer(modifier = Modifier.width(8.dp))
                                Text("Scanning")
                            } else {
                                Icon(Icons.Default.Refresh, contentDescription = null, modifier = Modifier.size(18.dp))
                                Spacer(modifier = Modifier.width(8.dp))
                                Text("Fast Scan")
                            }
                        }
                    }
                }

                // Libraries List Section
                Text(
                    text = "Your Libraries",
                    style = MaterialTheme.typography.titleLarge,
                    color = Color.White,
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.padding(top = 8.dp)
                )

                if (uiState.isLoading) {
                    Box(modifier = Modifier.fillMaxWidth().height(200.dp), contentAlignment = Alignment.Center) {
                        CircularProgressIndicator(color = PrimaryBlue)
                    }
                } else if (uiState.libraries.isEmpty()) {
                    Box(modifier = Modifier.fillMaxWidth().height(200.dp), contentAlignment = Alignment.Center) {
                        Text("No libraries found", color = GrayText)
                    }
                } else {
                    uiState.libraries.forEach { lib ->
                        LibraryItem(
                            name = lib.name,
                            path = lib.path,
                            type = lib.library_type,
                            onDelete = { viewModel.deleteLibrary(lib.id) }
                        )
                    }
                }

                uiState.error?.let {
                    Text(
                        text = "Error: $it",
                        color = Color.Red,
                        modifier = Modifier.padding(8.dp)
                    )
                }
                
                Spacer(modifier = Modifier.height(80.dp)) // Floating action button spacer
            }
        }
    }
}

@Composable
fun LibraryItem(
    name: String,
    path: String,
    type: String,
    onDelete: () -> Unit
) {
    GlassyCard(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp)
    ) {
        Row(
            modifier = Modifier
                .padding(16.dp)
                .fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = name,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Bold,
                    color = Color.White
                )
                Text(
                    text = type.replace("_", " ").uppercase(),
                    style = MaterialTheme.typography.labelSmall,
                    color = PrimaryBlue,
                    fontWeight = FontWeight.Bold
                )
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    text = path,
                    style = MaterialTheme.typography.bodySmall,
                    color = GrayText,
                    maxLines = 1
                )
            }
            
            IconButton(
                onClick = onDelete,
                colors = IconButtonDefaults.iconButtonColors(contentColor = Color.Red.copy(alpha = 0.7f))
            ) {
                Icon(Icons.Default.Delete, contentDescription = "Delete")
            }
        }
    }
}

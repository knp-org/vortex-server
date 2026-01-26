package org.knp.vortex.ui.screens.library

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Folder
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

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CreateLibraryScreen(
    onBack: () -> Unit,
    onSuccess: () -> Unit,
    viewModel: CreateLibraryViewModel = hiltViewModel()
) {
    val uiState by viewModel.uiState.collectAsState()

    LaunchedEffect(uiState.isSuccess) {
        if (uiState.isSuccess) {
            onSuccess()
            viewModel.resetState()
        }
    }
    
    if (uiState.showDirectoryPicker) {
        DirectoryPickerDialog(
            currentPath = uiState.directoryPath,
            directories = uiState.availableDirectories,
            isLoading = uiState.isDirectoryLoading,
            onDismiss = { viewModel.closeDirectoryPicker() },
            onSelectPath = { viewModel.selectDirectory(it) },
            onNavigate = { viewModel.navigateDirectory(it) }
        )
    }

    org.knp.vortex.ui.components.GlassyBackground {
        Scaffold(
            containerColor = Color.Transparent,
            topBar = {
                GlassyTopBar(title = "Create Library", onBack = onBack)
            }
        ) { padding ->
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding)
                    .padding(bottom = 16.dp) // Bottom padding only
                    .verticalScroll(rememberScrollState()), // Add scroll support
                verticalArrangement = Arrangement.spacedBy(16.dp)
            ) {
                Column(
                    modifier = Modifier.padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(16.dp)
                ) {
                    // Name Input
                    org.knp.vortex.ui.components.GlassyTextField(
                        value = uiState.name,
                        onValueChange = { viewModel.updateName(it) },
                        label = "Library Name",
                        modifier = Modifier.fillMaxWidth()
                    )

                    // Path Selection
                    Text("Folder Path", color = Color.White, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold)
                    
                    org.knp.vortex.ui.components.GlassyTextField(
                        value = uiState.path,
                        onValueChange = { viewModel.updatePath(it) },
                        label = "Server Path",
                        modifier = Modifier.fillMaxWidth(),
                        singleLine = true,
                        trailingIcon = {
                            IconButton(onClick = { viewModel.openDirectoryPicker() }) {
                                Icon(Icons.Default.Folder, contentDescription = "Browse", tint = PrimaryBlue)
                            }
                        }
                    )
                    Text("Click folder icon to browse server paths", color = GrayText, style = MaterialTheme.typography.bodySmall)

                    // Type Dropdown
                    Text("Library Type", color = Color.White, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold)
                    
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .horizontalScroll(rememberScrollState()),
                        horizontalArrangement = Arrangement.spacedBy(8.dp)
                    ) {
                        listOf("movies", "tv_shows", "music_videos", "other").forEach { type ->
                            val isSelected = uiState.type == type
                            FilterChip(
                                selected = isSelected,
                                onClick = { viewModel.updateType(type) },
                                label = { Text(type.replace("_", " ").capitalize()) },
                                colors = FilterChipDefaults.filterChipColors(
                                    selectedContainerColor = PrimaryBlue,
                                    containerColor = SurfaceColor,
                                    labelColor = Color.White,
                                    selectedLabelColor = Color.White
                                ),
                                border = null
                            )
                        }
                    }

                    if (uiState.error != null) {
                        Text(
                            text = uiState.error!!,
                            color = Color.Red,
                            style = MaterialTheme.typography.bodyMedium
                        )
                    }

                    Spacer(modifier = Modifier.height(24.dp))

                    Button(
                        onClick = { viewModel.createLibrary() },
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(56.dp),
                        colors = ButtonDefaults.buttonColors(containerColor = PrimaryBlue),
                        shape = RoundedCornerShape(12.dp),
                        enabled = !uiState.isLoading
                    ) {
                        if (uiState.isLoading) {
                            CircularProgressIndicator(color = Color.White, modifier = Modifier.size(24.dp))
                        } else {
                            Text("Finish and Create", fontWeight = FontWeight.Bold, fontSize = 16.sp)
                        }
                    }
                }
            }
        }
    }
}

private fun String.capitalize(): String {
    return this.replaceFirstChar { if (it.isLowerCase()) it.titlecase() else it.toString() }
}

package org.knp.vortex.ui.screens.settings

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Folder

import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import org.knp.vortex.ui.components.GlassyTopBar
import org.knp.vortex.ui.theme.*

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    onBack: () -> Unit,
    onManageLibraries: () -> Unit,
    viewModel: SettingsViewModel = hiltViewModel()
) {
    val uiState by viewModel.uiState.collectAsState()

    org.knp.vortex.ui.components.GlassyBackground {
        Scaffold(
            containerColor = Color.Transparent,
            topBar = {
                GlassyTopBar(title = "Settings", onBack = onBack)
            }
        ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(16.dp)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            // Server URL Section
            Text(
                text = "Server Configuration",
                style = MaterialTheme.typography.titleMedium,
                color = Color.White,
                fontWeight = FontWeight.Bold
            )

            org.knp.vortex.ui.components.GlassyTextField(
                value = uiState.serverUrl,
                onValueChange = { viewModel.updateServerUrl(it) },
                label = "Server URL",
                modifier = Modifier.fillMaxWidth()
            )

            Text(
                text = "Enter the full URL of your media server (e.g., http://192.168.1.100:3000)",
                style = MaterialTheme.typography.bodySmall,
                color = Color.Gray
            )

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                OutlinedButton(
                    onClick = { viewModel.resetToDefault() },
                    modifier = Modifier.weight(1f),
                    shape = RoundedCornerShape(12.dp),
                    colors = ButtonDefaults.outlinedButtonColors(
                        contentColor = Color.White
                    )
                ) {
                    Text("Reset to Default")
                }

                Button(
                    onClick = { viewModel.saveSettings() },
                    modifier = Modifier.weight(1f),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = PrimaryBlue
                    ),
                    shape = RoundedCornerShape(12.dp)
                ) {
                    Text("Save")
                }
            }

            if (uiState.isSaved) {
                Text(
                    text = "✓ Settings saved. Restart app to apply changes.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = Color(0xFF4CAF50)
                )
            }

            uiState.error?.let {
                Text(
                    text = "⚠ $it",
                    style = MaterialTheme.typography.bodyMedium,
                    color = Color.Red
                )
            }

            Spacer(modifier = Modifier.height(16.dp))

            // Library Management Section (Moved to 2nd position)
            Text(
                text = "Library Management",
                style = MaterialTheme.typography.titleMedium,
                color = Color.White,
                fontWeight = FontWeight.Bold
            )

            org.knp.vortex.ui.components.GlassyCard(
                onClick = onManageLibraries,
                modifier = Modifier.fillMaxWidth()
            ) {
                Row(
                    modifier = Modifier.padding(16.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Icon(
                        imageVector = Icons.Default.Folder,
                        contentDescription = null,
                        tint = PrimaryBlue,
                        modifier = Modifier.size(32.dp).padding(4.dp)
                    )
                    Spacer(modifier = Modifier.width(16.dp))
                    Column {
                        Text("Manage Libraries", color = Color.White, fontWeight = FontWeight.Bold)
                        Text("Add or remove media folders", color = GrayText, style = MaterialTheme.typography.bodySmall)
                    }
                }
            }
            
            Spacer(modifier = Modifier.height(16.dp))

            // Security Section
            Text(
                text = "Security",
                style = MaterialTheme.typography.titleMedium,
                color = Color.White,
                fontWeight = FontWeight.Bold
            )
            
            Row(
                modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = "Biometric Lock",
                        style = MaterialTheme.typography.bodyLarge,
                        color = Color.White
                    )
                    Text(
                        text = "Require authentication when opening the app",
                        style = MaterialTheme.typography.bodySmall,
                        color = Color.Gray
                    )
                }
                Switch(
                    checked = uiState.isBiometricEnabled,
                    onCheckedChange = { viewModel.toggleBiometric(it) },
                    colors = SwitchDefaults.colors(
                        checkedThumbColor = PrimaryBlue,
                        checkedTrackColor = PrimaryBlue.copy(alpha = 0.5f)
                    )
                )
            }

            Spacer(modifier = Modifier.height(16.dp))

            // Metadata Section
            Text(
                text = "Metadata Providers",
                style = MaterialTheme.typography.titleMedium,
                color = Color.White,
                fontWeight = FontWeight.Bold
            )

            org.knp.vortex.ui.components.GlassyTextField(
                value = uiState.tmdbApiKey,
                onValueChange = { viewModel.updateTmdbApiKey(it) },
                label = "TMDB API Key",
                modifier = Modifier.fillMaxWidth()
            )

            Spacer(modifier = Modifier.height(8.dp))

            var showResetDialog by remember { mutableStateOf(false) }

            if (showResetDialog) {
                org.knp.vortex.ui.components.GlassyDialog(
                    onDismissRequest = { showResetDialog = false },
                    title = "Reset Database?",
                    content = {
                        Text(
                            "This will clear all metadata and scanned libraries. This action cannot be undone.",
                            color = Color.White.copy(alpha = 0.8f),
                            style = MaterialTheme.typography.bodyMedium
                        )
                    },
                    confirmButton = {
                        Button(
                            onClick = { 
                                viewModel.resetDatabase() 
                                showResetDialog = false
                            },
                            colors = ButtonDefaults.buttonColors(containerColor = ErrorRed)
                        ) {
                            Text("Reset", color = Color.White)
                        }
                    },
                    dismissButton = {
                        TextButton(onClick = { showResetDialog = false }) {
                            Text("Cancel", color = Color.Gray)
                        }
                    }
                )
            }

            Button(
                onClick = { showResetDialog = true },
                modifier = Modifier.fillMaxWidth(),
                colors = ButtonDefaults.buttonColors(containerColor = Color.Red.copy(alpha = 0.5f)),
                shape = RoundedCornerShape(12.dp)
            ) {
                Text("Reset Database (Clear Metadata)", color = Color.White)
            }

            Spacer(modifier = Modifier.height(16.dp))

        }
        }
    }
}

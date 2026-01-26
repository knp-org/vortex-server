package org.knp.vortex.ui.screens.library

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import org.knp.vortex.data.remote.DirectoryEntryDto

@Composable
fun DirectoryPickerDialog(
    currentPath: String,
    directories: List<DirectoryEntryDto>,
    isLoading: Boolean,
    onDismiss: () -> Unit,
    onSelectPath: (String) -> Unit, // Confirm this path
    onNavigate: (String) -> Unit // Enter directory
) {
    Dialog(onDismissRequest = onDismiss) {
        Card(
            modifier = Modifier
                .fillMaxWidth()
                .height(500.dp)
                .padding(16.dp),
            shape = RoundedCornerShape(16.dp),
            colors = CardDefaults.cardColors(containerColor = Color(0xFF1E1E1E))
        ) {
            Column(modifier = Modifier.padding(16.dp)) {
                // Header
                Text(
                    text = "Select Server Folder",
                    style = MaterialTheme.typography.titleLarge,
                    color = Color.White
                )
                Spacer(modifier = Modifier.height(8.dp))
                
                // Current Path and Up Button
                Row(verticalAlignment = Alignment.CenterVertically) {
                    IconButton(onClick = { 
                        // Simplified Up navigation: just let View Model handle logic or simple string manipulation 
                        // But path is just a string. ViewModel knows logic.
                        // We will allow user to tap ".." in the list instead.
                        onNavigate("..")
                    }) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, "Up", tint = Color.Gray)
                    }
                    Text(
                        text = if (currentPath.isEmpty()) "Root" else currentPath,
                        style = MaterialTheme.typography.bodyMedium,
                        color = Color.LightGray,
                        modifier = Modifier.weight(1f)
                    )
                }
                
                HorizontalDivider(color = Color.Gray, thickness = 0.5.dp)
                
                // Directory List
                Box(modifier = Modifier.weight(1f)) {
                    if (isLoading) {
                        CircularProgressIndicator(modifier = Modifier.align(Alignment.Center))
                    } else {
                        LazyColumn {
                            items(directories) { dir ->
                                DirectoryItem(dir, onClick = { onNavigate(dir.path) })
                            }
                            if (directories.isEmpty()) {
                                item {
                                    Text("No folders found", color = Color.Gray, modifier = Modifier.padding(16.dp))
                                }
                            }
                        }
                    }
                }
                
                HorizontalDivider(color = Color.Gray, thickness = 0.5.dp)
                
                // Action Buttons
                Row(
                    modifier = Modifier.fillMaxWidth().padding(top = 16.dp),
                    horizontalArrangement = Arrangement.End
                ) {
                    TextButton(onClick = onDismiss) {
                        Text("Cancel", color = Color.Gray)
                    }
                    Spacer(modifier = Modifier.width(8.dp))
                    Button(
                        onClick = { onSelectPath(currentPath) },
                        colors = ButtonDefaults.buttonColors(containerColor = Color.Red)
                    ) {
                        Text("Select Current")
                    }
                }
            }
        }
    }
}

@Composable
fun DirectoryItem(entry: DirectoryEntryDto, onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(vertical = 12.dp, horizontal = 8.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(Icons.Default.Folder, contentDescription = null, tint = Color(0xFFFFC107))
        Spacer(modifier = Modifier.width(16.dp))
        Text(text = entry.name, color = Color.White)
    }
}

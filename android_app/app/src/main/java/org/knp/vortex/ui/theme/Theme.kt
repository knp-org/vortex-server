package org.knp.vortex.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

private val DarkColorScheme = darkColorScheme(
    primary = PrimaryBlue,
    secondary = CyanAccent,
    tertiary = GrayText,
    background = DeepBackground,
    surface = SurfaceColor,
    onPrimary = Color.White,
    onSecondary = DeepBackground,
    onTertiary = Color.White,
    onBackground = WhiteText,
    onSurface = WhiteText,
    error = ErrorRed
)

@Composable
fun MediaServerTheme(
    content: @Composable () -> Unit
) {
    // Always use dark theme for Netflix-like look
    MaterialTheme(
        colorScheme = DarkColorScheme,
        content = content
    )
}

package dev.ensub.android.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

private val EnsubColors = darkColorScheme(
    primary = Color(0xFF65D6A6),
    onPrimary = Color(0xFF06251A),
    secondary = Color(0xFFF1B85B),
    onSecondary = Color(0xFF2D1B00),
    background = Color(0xFF111315),
    onBackground = Color(0xFFF3F4F2),
    surface = Color(0xFF1A1D20),
    onSurface = Color(0xFFF3F4F2),
    surfaceVariant = Color(0xFF282D31),
    onSurfaceVariant = Color(0xFFB9C2C5),
    outline = Color(0xFF748085),
    outlineVariant = Color(0xFF343A3E),
    error = Color(0xFFFF8A80),
    onError = Color(0xFF3D0503),
)

@Composable
fun EnsubTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = EnsubColors,
        content = content,
    )
}

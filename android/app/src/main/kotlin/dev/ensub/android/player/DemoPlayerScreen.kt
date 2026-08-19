package dev.ensub.android.player

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import java.util.Locale

@Composable
fun DemoPlayerRoute(viewModel: DemoPlayerViewModel = viewModel()) {
    val state by viewModel.uiState.collectAsStateWithLifecycle()
    DemoPlayerScreen(
        state = state,
        onTogglePlayback = viewModel::togglePlayback,
        onSeek = viewModel::seekTo,
        onCueSelected = viewModel::seekToCue,
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DemoPlayerScreen(
    state: PlayerUiState,
    onTogglePlayback: () -> Unit,
    onSeek: (Long) -> Unit,
    onCueSelected: (Long) -> Unit,
) {
    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            TopAppBar(
                title = {
                    Text(
                        text = "Ensub",
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                    )
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.background,
                ),
            )
        },
    ) { contentPadding ->
        when {
            state.isLoading -> LoadingState(Modifier.padding(contentPadding))
            state.errorMessage != null -> ErrorState(
                message = state.errorMessage,
                modifier = Modifier.padding(contentPadding),
            )
            state.episode != null -> PlayerContent(
                state = state,
                onTogglePlayback = onTogglePlayback,
                onSeek = onSeek,
                onCueSelected = onCueSelected,
                modifier = Modifier.padding(contentPadding),
            )
        }
    }
}

@Composable
private fun PlayerContent(
    state: PlayerUiState,
    onTogglePlayback: () -> Unit,
    onSeek: (Long) -> Unit,
    onCueSelected: (Long) -> Unit,
    modifier: Modifier = Modifier,
) {
    val episode = checkNotNull(state.episode)
    Column(modifier = modifier.fillMaxSize()) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 20.dp, vertical = 12.dp),
        ) {
            Text(
                text = episode.feedTitle.uppercase(Locale.ROOT),
                color = MaterialTheme.colorScheme.secondary,
                style = MaterialTheme.typography.labelMedium,
            )
            Spacer(Modifier.height(4.dp))
            Text(
                text = episode.episodeTitle,
                style = MaterialTheme.typography.headlineSmall,
                fontWeight = FontWeight.SemiBold,
            )
        }

        Transport(
            state = state,
            onTogglePlayback = onTogglePlayback,
            onSeek = onSeek,
        )

        Text(
            text = "Transcript",
            style = MaterialTheme.typography.titleSmall,
            fontWeight = FontWeight.SemiBold,
            modifier = Modifier.padding(start = 20.dp, top = 18.dp, end = 20.dp, bottom = 8.dp),
        )
        HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
        LazyColumn(modifier = Modifier.fillMaxSize()) {
            items(state.cues, key = Cue::id) { cue ->
                val isActive = cue.index in state.activeCueIndices
                val isPreceding = state.activeCueIndices.isEmpty() &&
                    cue.index == state.precedingCueIndex
                CueRow(
                    cue = cue,
                    isActive = isActive,
                    isPreceding = isPreceding,
                    onClick = { onCueSelected(cue.index) },
                )
            }
        }
    }
}

@Composable
private fun Transport(
    state: PlayerUiState,
    onTogglePlayback: () -> Unit,
    onSeek: (Long) -> Unit,
) {
    val duration = state.durationMs.coerceAtLeast(1)
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surface)
            .padding(horizontal = 16.dp, vertical = 12.dp),
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.fillMaxWidth(),
        ) {
            IconButton(
                onClick = onTogglePlayback,
                modifier = Modifier
                    .size(48.dp)
                    .background(
                        color = MaterialTheme.colorScheme.primary,
                        shape = RoundedCornerShape(6.dp),
                    ),
            ) {
                Icon(
                    imageVector = if (state.isPlaying) Icons.Filled.Pause else Icons.Filled.PlayArrow,
                    contentDescription = if (state.isPlaying) "Pause" else "Play",
                    tint = MaterialTheme.colorScheme.onPrimary,
                )
            }
            Spacer(Modifier.width(14.dp))
            Slider(
                value = state.positionMs.coerceIn(0, duration).toFloat(),
                onValueChange = { onSeek(it.toLong()) },
                valueRange = 0f..duration.toFloat(),
                modifier = Modifier.weight(1f),
            )
        }
        Row(
            horizontalArrangement = Arrangement.SpaceBetween,
            modifier = Modifier
                .fillMaxWidth()
                .padding(start = 62.dp),
        ) {
            TimeLabel(state.positionMs)
            TimeLabel(state.durationMs)
        }
    }
}

@Composable
private fun CueRow(
    cue: Cue,
    isActive: Boolean,
    isPreceding: Boolean,
    onClick: () -> Unit,
) {
    val background = when {
        isActive -> MaterialTheme.colorScheme.primary.copy(alpha = 0.14f)
        isPreceding -> MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.55f)
        else -> Color.Transparent
    }
    Row(
        verticalAlignment = Alignment.Top,
        modifier = Modifier
            .fillMaxWidth()
            .background(background)
            .clickable(onClick = onClick)
            .semantics { selected = isActive }
            .padding(horizontal = 20.dp, vertical = 14.dp),
    ) {
        Box(
            modifier = Modifier
                .padding(top = 3.dp)
                .size(width = 3.dp, height = 38.dp)
                .background(
                    color = if (isActive) MaterialTheme.colorScheme.primary else Color.Transparent,
                    shape = RoundedCornerShape(2.dp),
                ),
        )
        Spacer(Modifier.width(12.dp))
        Text(
            text = formatTime(cue.startMs),
            color = if (isActive) {
                MaterialTheme.colorScheme.primary
            } else {
                MaterialTheme.colorScheme.onSurfaceVariant
            },
            fontFamily = FontFamily.Monospace,
            style = MaterialTheme.typography.labelMedium,
            modifier = Modifier.width(52.dp),
        )
        Text(
            text = cue.text,
            color = MaterialTheme.colorScheme.onSurface,
            style = MaterialTheme.typography.bodyLarge,
            fontWeight = if (isActive) FontWeight.Medium else FontWeight.Normal,
            modifier = Modifier.weight(1f),
        )
    }
    HorizontalDivider(
        color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.6f),
        modifier = Modifier.padding(start = 108.dp),
    )
}

@Composable
private fun TimeLabel(milliseconds: Long) {
    Text(
        text = formatTime(milliseconds),
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        fontFamily = FontFamily.Monospace,
        style = MaterialTheme.typography.labelMedium,
    )
}

@Composable
private fun LoadingState(modifier: Modifier = Modifier) {
    Box(modifier = modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        CircularProgressIndicator()
    }
}

@Composable
private fun ErrorState(message: String, modifier: Modifier = Modifier) {
    Box(
        modifier = modifier
            .fillMaxSize()
            .padding(24.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = message,
            color = MaterialTheme.colorScheme.error,
            style = MaterialTheme.typography.bodyLarge,
        )
    }
}

private fun formatTime(milliseconds: Long): String {
    val totalSeconds = milliseconds.coerceAtLeast(0) / 1_000
    return String.format(Locale.ROOT, "%d:%02d", totalSeconds / 60, totalSeconds % 60)
}

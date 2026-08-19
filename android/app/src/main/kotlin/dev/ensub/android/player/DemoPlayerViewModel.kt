package dev.ensub.android.player

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.exoplayer.ExoPlayer
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

data class PlaybackSnapshot(
    val positionMs: Long,
    val durationMs: Long,
    val isPlaying: Boolean,
)

data class PlayerUiState(
    val episode: Episode? = null,
    val cues: List<Cue> = emptyList(),
    val activeCueIndices: Set<Long> = emptySet(),
    val anchorCueIndex: Long? = null,
    val precedingCueIndex: Long? = null,
    val positionMs: Long = 0,
    val durationMs: Long = 0,
    val isPlaying: Boolean = false,
    val isLoading: Boolean = true,
    val errorMessage: String? = null,
)

internal fun mapPlayerState(
    engine: TranscriptEngine,
    snapshot: PlaybackSnapshot,
): PlayerUiState {
    val positionMs = snapshot.positionMs.coerceAtLeast(0)
    val durationMs = snapshot.durationMs.takeIf { it > 0 } ?: engine.episode.durationMs
    val sync = engine.syncAt(positionMs)
    return PlayerUiState(
        episode = engine.episode,
        cues = engine.cues,
        activeCueIndices = sync.activeCueIndices,
        anchorCueIndex = sync.anchorCueIndex,
        precedingCueIndex = sync.precedingCueIndex,
        positionMs = positionMs,
        durationMs = durationMs,
        isPlaying = snapshot.isPlaying,
        isLoading = false,
    )
}

class DemoPlayerViewModel(application: Application) : AndroidViewModel(application) {
    private val mutableState = MutableStateFlow(PlayerUiState())
    val uiState: StateFlow<PlayerUiState> = mutableState.asStateFlow()

    private var engine: TranscriptEngine? = null
    private var player: ExoPlayer? = null

    init {
        initialize(application)
    }

    fun togglePlayback() {
        player?.let { activePlayer ->
            if (activePlayer.isPlaying) {
                activePlayer.pause()
            } else {
                activePlayer.play()
            }
            updateState()
        }
    }

    fun seekTo(positionMs: Long) {
        player?.seekTo(positionMs.coerceAtLeast(0))
        updateState()
    }

    fun seekToCue(cueIndex: Long) {
        engine
            ?.cues
            ?.firstOrNull { it.index == cueIndex }
            ?.let { seekTo(it.startMs) }
    }

    override fun onCleared() {
        player?.release()
        player = null
        engine?.close()
        engine = null
        super.onCleared()
    }

    private fun initialize(application: Application) {
        try {
            val fixture = application.assets.open("demo-fixture.json").use { it.readBytes() }
            engine = UniFfiTranscriptEngine.fromFixture(SOURCE_URL, fixture)
            player = ExoPlayer.Builder(application).build().apply {
                setMediaItem(MediaItem.fromUri(DEMO_AUDIO_URI))
                prepare()
            }
            updateState()
            viewModelScope.launch {
                while (isActive) {
                    updateState()
                    delay(if (player?.isPlaying == true) PLAYING_POLL_MS else PAUSED_POLL_MS)
                }
            }
        } catch (error: Exception) {
            player?.release()
            player = null
            engine?.close()
            engine = null
            mutableState.value = PlayerUiState(
                isLoading = false,
                errorMessage = error.message ?: "Unable to load the bundled demo.",
            )
        }
    }

    private fun updateState() {
        val activeEngine = engine ?: return
        val activePlayer = player ?: return
        val playerDuration = activePlayer.duration.takeUnless { it == C.TIME_UNSET }
            ?: activeEngine.episode.durationMs
        mutableState.value = mapPlayerState(
            engine = activeEngine,
            snapshot = PlaybackSnapshot(
                positionMs = activePlayer.currentPosition,
                durationMs = playerDuration,
                isPlaying = activePlayer.isPlaying,
            ),
        )
    }

    private companion object {
        const val SOURCE_URL = "https://fixture.ensub.invalid/demo-fixture.json"
        const val DEMO_AUDIO_URI = "asset:///demo.mp3"
        const val PLAYING_POLL_MS = 100L
        const val PAUSED_POLL_MS = 250L
    }
}

package dev.ensub.android.player

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PlaybackStateMapperTest {
    private val episode = Episode("Synthetic Signal", "Listening", 120_000)
    private val cues = listOf(
        Cue(2, "cue-2", null, 19_000, 30_000, "First"),
        Cue(3, "cue-3", null, 29_000, 39_000, "Second"),
    )

    @Test
    fun overlap_maps_every_active_cue_and_player_transport_state() {
        val state = mapPlayerState(
            engine = FakeEngine(TranscriptSync(setOf(2, 3), 2, null)),
            snapshot = PlaybackSnapshot(positionMs = 29_500, durationMs = 120_000, isPlaying = true),
        )

        assertEquals(setOf(2L, 3L), state.activeCueIndices)
        assertEquals(2L, state.anchorCueIndex)
        assertEquals(29_500, state.positionMs)
        assertEquals(120_000, state.durationMs)
        assertTrue(state.isPlaying)
    }

    @Test
    fun gap_keeps_preceding_context_without_marking_a_cue_active() {
        val state = mapPlayerState(
            engine = FakeEngine(TranscriptSync(emptySet(), null, 3)),
            snapshot = PlaybackSnapshot(positionMs = 40_000, durationMs = 120_000, isPlaying = false),
        )

        assertTrue(state.activeCueIndices.isEmpty())
        assertEquals(3L, state.precedingCueIndex)
        assertFalse(state.isPlaying)
    }

    private inner class FakeEngine(
        private val sync: TranscriptSync,
    ) : TranscriptEngine {
        override val episode: Episode = this@PlaybackStateMapperTest.episode
        override val cues: List<Cue> = this@PlaybackStateMapperTest.cues

        override fun syncAt(positionMs: Long): TranscriptSync = sync

        override fun close() = Unit
    }
}

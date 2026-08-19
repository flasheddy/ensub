package dev.ensub.android.player

import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.ensub_uniffi.EpisodeDto
import uniffi.ensub_uniffi.TranscriptCueDto
import uniffi.ensub_uniffi.TranscriptSyncDto

class TranscriptEngineTest {
    @Test
    fun ffi_records_map_to_application_owned_models() {
        val episode = EpisodeDto("Synthetic Signal", "Listening", 120_000).toModel()
        val cue = TranscriptCueDto(2, "cue-2", "words-and-sound", 19_000, 30_000, "Text").toModel()
        val sync = TranscriptSyncDto(listOf(2, 3), 2, null).toModel()

        assertEquals(Episode("Synthetic Signal", "Listening", 120_000), episode)
        assertEquals(Cue(2, "cue-2", "words-and-sound", 19_000, 30_000, "Text"), cue)
        assertEquals(TranscriptSync(setOf(2, 3), 2, null), sync)
    }
}

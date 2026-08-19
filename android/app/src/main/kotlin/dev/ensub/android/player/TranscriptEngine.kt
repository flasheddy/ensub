package dev.ensub.android.player

import uniffi.ensub_uniffi.EpisodeDto
import uniffi.ensub_uniffi.TranscriptCueDto
import uniffi.ensub_uniffi.TranscriptSession
import uniffi.ensub_uniffi.TranscriptSyncDto

data class Episode(
    val feedTitle: String,
    val episodeTitle: String,
    val durationMs: Long,
)

data class Cue(
    val index: Long,
    val id: String,
    val sourceCueId: String?,
    val startMs: Long,
    val endMs: Long,
    val text: String,
)

data class TranscriptSync(
    val activeCueIndices: Set<Long>,
    val anchorCueIndex: Long?,
    val precedingCueIndex: Long?,
)

interface TranscriptEngine : AutoCloseable {
    val episode: Episode
    val cues: List<Cue>

    fun syncAt(positionMs: Long): TranscriptSync
}

class UniFfiTranscriptEngine private constructor(
    private val session: TranscriptSession,
) : TranscriptEngine {
    override val episode: Episode = session.episode().toModel()
    override val cues: List<Cue> = session.cues().map(TranscriptCueDto::toModel)

    override fun syncAt(positionMs: Long): TranscriptSync = session.syncAt(positionMs).toModel()

    override fun close() {
        session.close()
    }

    companion object {
        fun fromFixture(sourceUrl: String, fixtureBytes: ByteArray): UniFfiTranscriptEngine =
            UniFfiTranscriptEngine(TranscriptSession.fromFixture(sourceUrl, fixtureBytes))
    }
}

internal fun EpisodeDto.toModel(): Episode = Episode(
    feedTitle = feedTitle,
    episodeTitle = episodeTitle,
    durationMs = durationMs,
)

internal fun TranscriptCueDto.toModel(): Cue = Cue(
    index = index,
    id = id,
    sourceCueId = sourceCueId,
    startMs = startMs,
    endMs = endMs,
    text = text,
)

internal fun TranscriptSyncDto.toModel(): TranscriptSync = TranscriptSync(
    activeCueIndices = activeCueIndices.toSet(),
    anchorCueIndex = anchorCueIndex,
    precedingCueIndex = precedingCueIndex,
)

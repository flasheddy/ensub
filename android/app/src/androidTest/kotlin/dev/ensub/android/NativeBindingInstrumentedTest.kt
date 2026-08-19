package dev.ensub.android

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import dev.ensub.android.player.UniFfiTranscriptEngine
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class NativeBindingInstrumentedTest {
    @Test
    fun bundled_fixture_crosses_uniffi_and_preserves_overlap_policy() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val fixture = context.assets.open("demo-fixture.json").use { it.readBytes() }

        UniFfiTranscriptEngine.fromFixture(SOURCE_URL, fixture).use { engine ->
            assertEquals(12, engine.cues.size)
            assertEquals(setOf(2L, 3L), engine.syncAt(29_500).activeCueIndices)
        }
    }

    private companion object {
        const val SOURCE_URL = "https://fixture.ensub.invalid/demo-fixture.json"
    }
}

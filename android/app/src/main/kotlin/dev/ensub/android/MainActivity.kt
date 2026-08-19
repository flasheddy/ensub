package dev.ensub.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import dev.ensub.android.player.DemoPlayerRoute
import dev.ensub.android.ui.theme.EnsubTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            EnsubTheme {
                DemoPlayerRoute()
            }
        }
    }
}

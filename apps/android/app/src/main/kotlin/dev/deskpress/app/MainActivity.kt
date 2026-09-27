package dev.deskpress.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.getValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel

/** The pack this build opens: a folder of the repo's examples, shipped as assets. */
private const val PACK = "one-day"

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            val model = viewModel { PackViewModel({ assets.pack(PACK) }) }
            val state by model.state.collectAsStateWithLifecycle()
            MaterialTheme { PackScreen(state) }
        }
    }
}

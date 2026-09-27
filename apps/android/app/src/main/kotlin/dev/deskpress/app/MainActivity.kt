package dev.deskpress.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.getValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import java.time.LocalDateTime

/** The pack this build opens: a folder of the repo's examples, shipped as assets. */
private const val PACK = "one-day"

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        // The e2e flows freeze the clock at a pack-local moment, the extra "now".
        val frozen = intent.getStringExtra("now")?.let(LocalDateTime::parse)
        setContent {
            val model = viewModel {
                val files = { assets.pack(PACK) }
                if (frozen == null) PackViewModel(files) else PackViewModel(files, { frozen })
            }
            val state by model.state.collectAsStateWithLifecycle()
            MaterialTheme { PackScreen(state, model::act) }
        }
    }
}

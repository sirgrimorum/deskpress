plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kover)
}

composeCompiler {
    stabilityConfigurationFiles.add(layout.projectDirectory.file("compose-stability.conf"))
}

// The repo root: the examples ship as assets, and the JVM tests load the desk build of the engine.
val repo: File = rootDir.resolve("../..")

android {
    namespace = "dev.deskpress.app"
    compileSdk {
        version = release(37) { minorApiLevel = 2 }
    }

    defaultConfig {
        applicationId = "dev.deskpress.app"
        minSdk = 28
        targetSdk = 37
        versionCode = 1
        versionName = "0.0.0"
        // `ABIS` in the Makefile, the only ones the engine is built for: no other could run the app.
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
    }

    buildFeatures {
        compose = true
    }

    sourceSets {
        getByName("main") {
            // Written by `make bindings`, along with jniLibs: the UniFFI bindings.
            kotlin.srcDir("src/generated/kotlin")
            assets.srcDir(repo.resolve("examples"))
        }
    }

    testOptions {
        unitTests.all {
            it.jvmArgs("--enable-native-access=ALL-UNNAMED")
            it.systemProperty("jna.library.path", repo.resolve("target/debug").path)
            it.systemProperty("deskpress.examples", repo.resolve("examples").path)
        }
    }
}

dependencies {
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.material3)
    implementation(libs.activity.compose)
    implementation(libs.biometric)
    implementation(libs.core.ktx)
    implementation(libs.fragment)
    implementation(libs.lifecycle.viewmodel.compose)
    implementation(libs.lifecycle.runtime.compose)
    implementation(libs.documentfile)
    implementation(libs.genai.prompt)
    implementation(libs.maplibre)
    implementation("net.java.dev.jna:jna:${libs.versions.jna.get()}@aar")

    testImplementation(libs.jna)
    testImplementation(libs.junit)
    testImplementation(libs.coroutines.test)
}

// Coverage of the code that is not drawing: the view model, the pack files, facts, sync,
// calendar, location, maps and theme. Screens, the activity and the generated bindings are left to
// the e2e flows, and keeping the maps offline to a phone. The floor is the level measured when it was set: what stays uncovered needs a
// device or a composition (the calendar provider, the Keystore, a picked folder, the network,
// the state a screen holds).
kover {
    reports {
        filters {
            excludes {
                classes(
                    "dev.deskpress.app.MainActivity*",
                    "dev.deskpress.app.Assistant",
                    "dev.deskpress.app.Assistant\$Companion",
                    "dev.deskpress.app.Asking*",
                    "dev.deskpress.app.Syncing*",
                    "dev.deskpress.app.Menu",
                    "dev.deskpress.app.ScreenKt*",
                    "dev.deskpress.app.SettingsKt*",
                    "dev.deskpress.app.Shell",
                    "dev.deskpress.app.DesignKt*",
                    "dev.deskpress.app.ContentRows",
                    "dev.deskpress.app.DocumentKt*",
                    "dev.deskpress.app.Pdf*",
                    "dev.deskpress.app.StyleKt*",
                    "dev.deskpress.app.GuideKt*",
                    "dev.deskpress.app.ComposableSingletons*",
                    "dev.deskpress.app.BuildConfig",
                    "dev.deskpress.engine.*",
                )
                annotatedBy("androidx.compose.runtime.Composable")
            }
        }
        // Just under what the tests reach, so a change that leaves new code untested fails here.
        verify { rule { minBound(82) } }
    }
}

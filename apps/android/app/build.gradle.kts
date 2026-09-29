plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kover)
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
    }

    buildFeatures {
        compose = true
    }

    sourceSets {
        getByName("main") {
            // Written by scripts/android.sh, along with jniLibs: the UniFFI bindings.
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
    implementation("net.java.dev.jna:jna:${libs.versions.jna.get()}@aar")

    testImplementation(libs.jna)
    testImplementation(libs.junit)
    testImplementation(libs.coroutines.test)
}

// Coverage of the code that is not drawing: the view model, the pack files, facts, sync,
// calendar, location and theme. Screens, the activity and the generated bindings are left to
// the e2e flows. The floor is the level measured when it was set: what stays uncovered needs a
// device (the calendar provider, the Keystore, a picked folder, the network).
kover {
    reports {
        filters {
            excludes {
                classes(
                    "dev.deskpress.app.MainActivity*",
                    "dev.deskpress.app.Asking*",
                    "dev.deskpress.app.Syncing*",
                    "dev.deskpress.app.Menu",
                    "dev.deskpress.app.ScreenKt*",
                    "dev.deskpress.app.DesignKt*",
                    "dev.deskpress.app.DocumentKt*",
                    "dev.deskpress.app.Pdf*",
                    "dev.deskpress.app.StyleKt*",
                    "dev.deskpress.app.ComposableSingletons*",
                    "dev.deskpress.app.BuildConfig",
                    "dev.deskpress.engine.*",
                )
                annotatedBy("androidx.compose.runtime.Composable")
            }
        }
        // Just under what the tests reach, so a change that leaves new code untested fails here.
        verify { rule { minBound(78) } }
    }
}

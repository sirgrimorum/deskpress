plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
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
    implementation(libs.lifecycle.viewmodel.compose)
    implementation(libs.lifecycle.runtime.compose)
    implementation("net.java.dev.jna:jna:${libs.versions.jna.get()}@aar")

    testImplementation(libs.jna)
    testImplementation(libs.junit)
    testImplementation(libs.coroutines.test)
}

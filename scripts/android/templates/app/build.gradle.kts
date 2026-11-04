import java.io.File

plugins { id("com.android.application")@APP_PLUGINS@ }

val signingNames = listOf("TAIM_KEYSTORE", "TAIM_KEY_ALIAS", "TAIM_STORE_PASSWORD", "TAIM_KEY_PASSWORD")
val signedRelease = signingNames.all { !System.getenv(it).isNullOrBlank() }
android {
    namespace = "@PACKAGE@"
    compileSdk = 36
    ndkVersion = "28.2.13676358"
    defaultConfig {
        applicationId = "@PACKAGE@"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
        // Secrets come from the build environment, never from committed files.
        manifestPlaceholders["taimMapsApiKey"] = System.getenv("TAIM_MAPS_API_KEY") ?: ""
    }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_1_8; targetCompatibility = JavaVersion.VERSION_1_8 }
    packaging { jniLibs { useLegacyPackaging = false } }
    signingConfigs {
        if (signedRelease) create("release") {
            storeFile = file(System.getenv("TAIM_KEYSTORE"))
            keyAlias = System.getenv("TAIM_KEY_ALIAS")
            storePassword = System.getenv("TAIM_STORE_PASSWORD")
            keyPassword = System.getenv("TAIM_KEY_PASSWORD")
        }
    }
    buildTypes {
        getByName("release") {
            isDebuggable = false
            isMinifyEnabled = false // JNI host names must remain stable.
            if (signedRelease) signingConfig = signingConfigs.getByName("release")
        }
    }
    sourceSets.getByName("debug").jniLibs.srcDir(layout.buildDirectory.dir("rust/debug"))
    sourceSets.getByName("release").jniLibs.srcDir(layout.buildDirectory.dir("rust/release"))
}

val requireReleaseSigning = tasks.register("requireReleaseSigning") {
    doLast { check(signedRelease) { "Release signing requires TAIM_KEYSTORE, TAIM_KEY_ALIAS, TAIM_STORE_PASSWORD, TAIM_KEY_PASSWORD; no debug-key fallback." } }
}
fun nativeBuild(profile: String) = tasks.register<Exec>("rust" + profile.replaceFirstChar { it.uppercaseChar() }) {
    workingDir(rootProject.projectDir)
    environment("ANDROID_NDK_HOME", File(android.sdkDirectory, "ndk/28.2.13676358").absolutePath)
    commandLine("python3", rootProject.file("tools/build-rust.py"), profile, layout.buildDirectory.dir("rust/$profile").get().asFile)
    // Always invoke Cargo: it tracks transitive path-dependent source changes itself.
}
val rustDebug = nativeBuild("debug")
val rustRelease = nativeBuild("release")
rustRelease.configure { dependsOn(requireReleaseSigning) }
tasks.matching { it.name == "preDebugBuild" }.configureEach { dependsOn(rustDebug) }
tasks.matching { it.name == "preReleaseBuild" }.configureEach { dependsOn(rustRelease) }

// A successful release task must pass the full SDK/API/permission lint suite.
// lintVital alone checks a smaller fatal subset and missed compatibility errors during audit.
tasks.matching { it.name == "assembleRelease" || it.name == "bundleRelease" }.configureEach {
    dependsOn("lintRelease")
}

dependencies {
@DEPENDENCIES@
}

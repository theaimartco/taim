// Packages the `mobile_shell` Rust library as an Android application.
//
// The Rust code is built by `cargo ndk` before every Gradle build and
// copied into `src/main/jniLibs`; there is no Java or Kotlin code, because
// the activity is the platform's own `android.app.NativeActivity`.
// See docs/platform/android.md.

plugins {
    id("com.android.application")
}

android {
    namespace = "com.theaimartco.taim.mobileshell"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.theaimartco.taim.mobileshell"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            // Replace with your own signing configuration before publishing.
            signingConfig = signingConfigs.getByName("debug")
        }
    }

    sourceSets["main"].jniLibs.srcDirs("src/main/jniLibs")
}

// The workspace root, three directories above this Gradle project.
val workspace = rootProject.projectDir.resolve("../../..").canonicalFile
val profile = if (gradle.startParameter.taskNames.any { it.contains("Release") }) "release" else "dev"

val cargoBuild by tasks.registering(Exec::class) {
    description = "Builds the Rust library for every Android ABI with cargo-ndk."
    workingDir = workspace
    commandLine(
        "cargo", "ndk",
        "--platform", "26",
        "-t", "arm64-v8a",
        "-t", "x86_64",
        "-o", projectDir.resolve("src/main/jniLibs").absolutePath,
        "build", "-p", "mobile_shell", "--lib", "--profile", profile,
    )
}

tasks.named("preBuild") {
    dependsOn(cargoBuild)
}

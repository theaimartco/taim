#!/usr/bin/env bash
# Installs everything a cloud session needs to build TAIM's Android
# applications: Rust Android targets, cargo-ndk, Gradle, the Android SDK
# (platform, build tools, platform tools) and the NDK.
#
#   bash scripts/cloud/setup-android.sh
#
# Safe to run again: what is already installed is kept. Meant for the cloud
# environment's setup script, so every new session starts ready:
#
#   bash /home/user/taim/scripts/cloud/setup-android.sh
#
# Hosts it downloads from: dl.google.com (SDK and NDK, and later the Android
# Gradle Plugin, which maven.google.com redirects to), services.gradle.org
# (Gradle), index.crates.io (cargo-ndk). The environment's network policy
# must allow them. Writes ~/.taim-android-env; source it before building:
#
#   . ~/.taim-android-env
set -euo pipefail

ANDROID_HOME="${ANDROID_HOME:-$HOME/android-sdk}"
GRADLE_VERSION="8.10.2"
GRADLE_HOME="$HOME/.local/gradle-$GRADLE_VERSION"
CMDLINE_TOOLS="commandlinetools-linux-11076708_latest.zip"
PLATFORM="android-35"
BUILD_TOOLS="35.0.0"
NDK_VERSION="27.2.12479018"
ENV_FILE="$HOME/.taim-android-env"

say() { printf '\n== %s\n' "$*"; }

need_host() {
  local url="$1"
  if ! curl -sS -o /dev/null -I --max-time 20 "$url" 2>/dev/null; then
    echo "Cannot reach $url." >&2
    echo "Allow its host in the cloud environment's network settings, then run this again." >&2
    exit 3
  fi
}

say "Rust Android targets"
rustup target add aarch64-linux-android x86_64-linux-android

say "cargo-ndk"
if ! command -v cargo-ndk >/dev/null 2>&1; then
  cargo install cargo-ndk --locked
fi
cargo ndk --version

say "Java"
if ! java -version 2>&1 | grep -qE 'version "(1[7-9]|2[0-9])'; then
  apt-get install -y openjdk-21-jdk-headless
fi

say "Gradle $GRADLE_VERSION"
if [ ! -x "$GRADLE_HOME/bin/gradle" ]; then
  need_host "https://services.gradle.org/distributions/"
  scratch="$(mktemp -d)"
  curl -sSfL -o "$scratch/gradle.zip" \
    "https://services.gradle.org/distributions/gradle-$GRADLE_VERSION-bin.zip"
  mkdir -p "$HOME/.local"
  unzip -q "$scratch/gradle.zip" -d "$HOME/.local"
  rm -rf "$scratch"
fi

say "Android SDK and NDK in $ANDROID_HOME"
SDKMANAGER="$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager"
if [ ! -x "$SDKMANAGER" ]; then
  need_host "https://dl.google.com/android/repository/$CMDLINE_TOOLS"
  scratch="$(mktemp -d)"
  curl -sSfL -o "$scratch/tools.zip" "https://dl.google.com/android/repository/$CMDLINE_TOOLS"
  mkdir -p "$ANDROID_HOME/cmdline-tools"
  unzip -q "$scratch/tools.zip" -d "$scratch"
  rm -rf "$ANDROID_HOME/cmdline-tools/latest"
  mv "$scratch/cmdline-tools" "$ANDROID_HOME/cmdline-tools/latest"
  rm -rf "$scratch"
fi

need_host "https://dl.google.com/android/repository/repository2-3.xml"
yes | "$SDKMANAGER" --sdk_root="$ANDROID_HOME" --licenses >/dev/null
"$SDKMANAGER" --sdk_root="$ANDROID_HOME" --install \
  "platform-tools" "platforms;$PLATFORM" "build-tools;$BUILD_TOOLS" "ndk;$NDK_VERSION"

cat >"$ENV_FILE" <<ENV
# Written by taim/scripts/cloud/setup-android.sh
export ANDROID_HOME="$ANDROID_HOME"
export ANDROID_SDK_ROOT="$ANDROID_HOME"
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/$NDK_VERSION"
export PATH="$GRADLE_HOME/bin:$ANDROID_HOME/platform-tools:$ANDROID_HOME/cmdline-tools/latest/bin:\$PATH"
ENV

say "Ready"
echo "Before building for Android, run: . $ENV_FILE"

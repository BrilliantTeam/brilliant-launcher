#!/usr/bin/env bash
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
crate="$here/../.."
sdk="${ANDROID_HOME:-$LOCALAPPDATA/Android/Sdk}"
ndk="${NDK_HOME:-$(ls -d "$sdk"/ndk/* | sort -V | tail -1)}"
bin="$ndk/toolchains/llvm/prebuilt/windows-x86_64/bin"
out="${1:-$HOME/Downloads/BrilliantLauncher-Android.apk}"

export CC_aarch64_linux_android="$bin/aarch64-linux-android24-clang.cmd"
export CXX_aarch64_linux_android="$bin/aarch64-linux-android24-clang++.cmd"
export AR_aarch64_linux_android="$bin/llvm-ar.exe"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$bin/aarch64-linux-android24-clang.cmd"

(cd "$crate" && cargo build --release --lib --target aarch64-linux-android --features tauri/custom-protocol)
mkdir -p "$here/app/src/main/jniLibs/arm64-v8a"
cp "$crate/target/aarch64-linux-android/release/libbrilliant_launcher_lib.so" "$here/app/src/main/jniLibs/arm64-v8a/"

export ANDROID_HOME="$sdk"
export JAVA_HOME="${GRADLE_JAVA_HOME:-/c/Program Files/Java/jdk-21.0.10}"
mode="${BUILD_TYPE:-debug}"
Mode="$(tr a-z A-Z <<< "${mode:0:1}")${mode:1}"
apk="$here/app/build/outputs/apk/arm64/$mode/app-arm64-$mode.apk"
rm -f "$apk"
(cd "$here" && ./gradlew.bat ":app:assembleArm64$Mode" -x "rustBuildArm64$Mode" --rerun-tasks -q)
cp "$apk" "$out"
ls -la "$out"

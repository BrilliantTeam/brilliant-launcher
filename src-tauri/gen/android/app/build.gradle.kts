plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

val appVersion = Regex("\"version\"\\s*:\\s*\"([^\"]+)\"").find(rootProject.file("../../tauri.conf.json").readText())!!.groupValues[1]

fun versionCodeOf(version: String): Int {
    val match = Regex("^(\\d+)\\.(\\d+)\\.(\\d+)(?:-(alpha|beta)\\.(\\d+))?$").matchEntire(version)
        ?: throw GradleException("Unsupported version $version")
    val (major, minor, patch, stage, n) = match.destructured
    val channel = when (stage) {
        "alpha" -> 100 + n.toInt()
        "beta" -> 300 + n.toInt()
        else -> 900
    }
    return ((major.toInt() * 100 + minor.toInt()) * 100 + patch.toInt()) * 1000 + channel
}

android {
    compileSdk = 36
    namespace = "net.brilliantw.launcher"
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        applicationId = "net.brilliantw.launcher"
        minSdk = 24
        targetSdk = 36
        versionCode = versionCodeOf(appVersion)
        versionName = appVersion
    }
    signingConfigs {
        create("release") {
            System.getenv("BRILLIANT_KEYSTORE")?.let { storeFile = file(it) }
            storePassword = System.getenv("BRILLIANT_KEYSTORE_PASSWORD")
            keyAlias = System.getenv("BRILLIANT_KEY_ALIAS")
            keyPassword = System.getenv("BRILLIANT_KEY_PASSWORD")
        }
    }
    buildTypes {
        getByName("debug") {
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
        }
        getByName("release") {
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("release")
        }
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }
    defaultConfig {
        ndk { abiFilters += "arm64-v8a" }
    }
    packaging {
        jniLibs.useLegacyPackaging = true
        jniLibs.pickFirsts += listOf("**/libbytehook.so", "**/libc++_shared.so", "**/libglxshim.so")
        jniLibs.excludes += listOf("**/libzink_dri.so", "**/libEGL_mesa.so", "**/libglapi.so", "**/libGLESv2_angle.so", "**/libEGL_angle.so", "**/libVkLayer_khronos_timeline_semaphore.so")
    }
    androidResources {
        ignoreAssetsPatterns += listOf("!<dir>armeabi-v7a", "!<dir>x86", "!<dir>x86_64", "!amethyst.png")
    }
    buildFeatures {
        buildConfig = true
    }
    if (System.getenv("BRILLIANT_PLAY") != null) {
        sourceSets.getByName("release").manifest.srcFile("src/play/AndroidManifest.xml")
    }
}

rust {
    rootDirRel = "../../../"
}

dependencies {
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.lifecycle:lifecycle-process:2.10.0")
    val runtime = providers.gradleProperty("brilliantRuntimeDir").getOrElse("../../../../../brilliant-mobile-runtime")
    implementation(files("$runtime/brilliant_runtime/build/outputs/aar/brilliant_runtime-release.aar"))
    implementation(files("$runtime/androidnsbypass/build/outputs/aar/androidnsbypass-release.aar"))
    implementation(fileTree("$runtime/app_pojavlauncher/libs") { include("*.aar") })
    implementation("javax.annotation:javax.annotation-api:1.3.2")
    implementation("commons-codec:commons-codec:1.15")
    implementation("androidx.preference:preference:1.2.0")
    implementation("androidx.drawerlayout:drawerlayout:1.2.0")
    implementation("androidx.viewpager2:viewpager2:1.1.0-beta01")
    implementation("androidx.constraintlayout:constraintlayout:2.1.4")
    implementation("com.github.duanhong169:checkerboarddrawable:1.0.2")
    implementation("com.github.PojavLauncherTeam:portrait-sdp:ed33e89cbc")
    implementation("com.github.PojavLauncherTeam:portrait-ssp:6c02fd739b")
    implementation("com.github.Mathias-Boulay:ExtendedView:1.0.0")
    implementation("com.github.Mathias-Boulay:android_gamepad_remapper:2.0.3")
    implementation("com.github.Mathias-Boulay:virtual-joystick-android:1.14")
    implementation("org.tukaani:xz:1.8")
    implementation("net.sourceforge.htmlcleaner:htmlcleaner:2.6.1")
    implementation("com.bytedance:bytehook:1.0.10")
    implementation("top.fifthlight.touchcontroller:proxy-client-android:0.0.4")
    implementation("net.java.dev.jna:jna:5.14.0@aar")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

tasks.matching { it.name.endsWith("AarMetadata") }.configureEach { enabled = false }

apply(from = "tauri.build.gradle.kts")
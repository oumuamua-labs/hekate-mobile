// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

val proverManifest = file("../../prover/Cargo.toml")

val hekateVersion = Regex("""hekate-prover-sys\s*=\s*"=?([0-9][0-9A-Za-z.+-]*)"""")
    .find(proverManifest.readText())
    ?.groupValues
    ?.get(1)
    ?: error("no hekate-prover-sys version in $proverManifest")

android {
    namespace = "dev.oumuamua.hekate.examples.signatures"
    compileSdk = 34

    defaultConfig {
        applicationId = "dev.oumuamua.hekate.examples.signatures"
        minSdk = 31
        targetSdk = 34
        versionCode = 1
        versionName = "0.1.0"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        buildConfigField("String", "HEKATE_VERSION", "\"$hekateVersion\"")

        ndk {
            abiFilters += listOf("arm64-v8a")
        }
    }

    buildFeatures {
        buildConfig = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }
}

dependencies {
    implementation("dev.oumuamua.hekate:hekate:0.3.0")
    implementation(files("../../prover/target/aar/signature-prover-release.aar"))
    implementation("net.java.dev.jna:jna:5.19.1@aar")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")

    androidTestImplementation("androidx.test:runner:1.6.2")
    androidTestImplementation("androidx.test.ext:junit:1.2.1")
    androidTestImplementation("androidx.test.uiautomator:uiautomator:2.3.0")
}
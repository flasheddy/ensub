import org.gradle.api.tasks.Exec

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
}

val generatedJniDir = layout.buildDirectory.dir("generated/jniLibs")
val generatedUniFfiDir = layout.buildDirectory.dir("generated/source/uniffi/kotlin")

android {
    namespace = "dev.ensub.android"
    compileSdk {
        version = release(37) {
            minorApiLevel = 0
        }
    }

    defaultConfig {
        applicationId = "dev.ensub.android"
        minSdk = 26
        targetSdk = 37
        versionCode = 1
        versionName = "0.1-spike"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        ndk {
            abiFilters += setOf("arm64-v8a", "x86_64")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        compose = true
        buildConfig = false
    }

    sourceSets {
        getByName("main") {
            assets.srcDir(rootProject.file("../crates/web_player/assets"))
            jniLibs.srcDir(generatedJniDir)
            kotlin.srcDir(generatedUniFfiDir)
        }
    }

    packaging {
        resources.excludes += "/META-INF/{AL2.0,LGPL2.1}"
        jniLibs.useLegacyPackaging = true
    }
}

kotlin {
    jvmToolchain(17)
}

val buildRustBindings by tasks.registering(Exec::class) {
    val repositoryRoot = rootProject.projectDir.parentFile
    workingDir(repositoryRoot)
    commandLine(
        "sh",
        repositoryRoot.resolve("scripts/build-android-bindings.sh"),
        generatedJniDir.get().asFile,
        generatedUniFfiDir.get().asFile,
    )
    inputs.files(
        repositoryRoot.resolve("Cargo.toml"),
        repositoryRoot.resolve("Cargo.lock"),
    )
    inputs.dir(repositoryRoot.resolve("bindings/ensub-uniffi/src"))
    inputs.file(repositoryRoot.resolve("bindings/ensub-uniffi/uniffi.toml"))
    outputs.dir(generatedJniDir)
    outputs.dir(generatedUniFfiDir)
}

tasks.named("preBuild").configure {
    dependsOn(buildRustBindings)
}

dependencies {
    implementation(platform(libs.compose.bom))
    androidTestImplementation(platform(libs.compose.bom))

    implementation(libs.activity.compose)
    implementation(libs.compose.material3)
    implementation(libs.compose.material.icons)
    implementation(libs.compose.material.icons.extended)
    implementation(libs.compose.ui)
    implementation(libs.compose.ui.tooling.preview)
    implementation(libs.coroutines.android)
    implementation(libs.lifecycle.runtime.compose)
    implementation(libs.lifecycle.viewmodel.compose)
    implementation(libs.media3.exoplayer)
    implementation("net.java.dev.jna:jna:5.19.1@aar")

    debugImplementation(libs.compose.ui.tooling)
    testImplementation(libs.junit)
    androidTestImplementation(libs.androidx.junit)
    androidTestImplementation(libs.espresso.core)
    androidTestImplementation(libs.test.runner)
}

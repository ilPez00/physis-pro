plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "dev.physis.monitor"
    compileSdk = 35

    defaultConfig {
        applicationId = "dev.physis.monitor"
        // 26: the oldest release where a foreground service behaves the way
        // this app depends on. Older devices would need a different design, and
        // pretending otherwise would ship a monitor that quietly stops.
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
    }

    buildTypes {
        release {
            // No shrinking for the first release: an APK built from git should
            // be diffable against its source without a mapping file.
            isMinifyEnabled = false
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
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
    // Deliberately minimal. No Play Services, no Firebase, no analytics SDK,
    // no JSON library (org.json is in AOSP) and no HTTP client (HttpURLConnection
    // is too). Every dependency is a thing a reviewer — and F-Droid, if that
    // road reopens — would have to audit, and none of these buy anything here.
    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.appcompat:appcompat:1.7.0")
}

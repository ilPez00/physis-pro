// Deliberately no Google Play Services repository beyond what AGP itself needs:
// nothing in this app links against Play Services. LocationManager,
// SensorManager and AudioRecord are all AOSP, which is what keeps the same
// build shippable from git, from Play, and from F-Droid if that road reopens.
pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "physis-monitor"
include(":app")

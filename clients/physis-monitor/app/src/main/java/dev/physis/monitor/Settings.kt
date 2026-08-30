package dev.physis.monitor

import android.content.Context
import android.provider.Settings.Secure

/**
 * Everything the app remembers, which is deliberately little: where to sync,
 * who to sync as, and which sensors the user turned on.
 *
 * No observations are stored here. Samples live in memory until they sync and
 * are dropped once the engine has them — the phone is a sensor, not an archive,
 * and an archive on the phone is a second copy of a life to lose.
 */
class Settings(context: Context) {

    private val prefs = context.getSharedPreferences("physis-monitor", Context.MODE_PRIVATE)

    var baseUrl: String
        get() = prefs.getString("base_url", "").orEmpty()
        set(v) = prefs.edit().putString("base_url", v.trim()).apply()

    var token: String
        get() = prefs.getString("token", "").orEmpty()
        set(v) = prefs.edit().putString("token", v.trim()).apply()

    var user: String
        get() = prefs.getString("user", "").orEmpty()
        set(v) = prefs.edit().putString("user", v.trim()).apply()

    /** Off by default, every one of them. A monitor that starts monitoring
     *  because it was installed is the thing people are right to distrust. */
    var steps: Boolean
        get() = prefs.getBoolean("steps", false)
        set(v) = prefs.edit().putBoolean("steps", v).apply()

    var location: Boolean
        get() = prefs.getBoolean("location", false)
        set(v) = prefs.edit().putBoolean("location", v).apply()

    var audio: Boolean
        get() = prefs.getBoolean("audio", false)
        set(v) = prefs.edit().putBoolean("audio", v).apply()

    var battery: Boolean
        get() = prefs.getBoolean("battery", false)
        set(v) = prefs.edit().putBoolean("battery", v).apply()

    /** Whether the user had monitoring running, so a reboot can restore what
     *  they chose — and only what they chose. */
    var running: Boolean
        get() = prefs.getBoolean("running", false)
        set(v) = prefs.edit().putBoolean("running", v).apply()

    var lastSync: String
        get() = prefs.getString("last_sync", "never").orEmpty()
        set(v) = prefs.edit().putString("last_sync", v).apply()

    /**
     * A stable name for this device.
     *
     * ANDROID_ID is per-app-per-device and resets on factory reset, so it
     * identifies "the phone that reported this" without being a cross-app
     * identifier anyone could correlate. The engine only needs to tell two
     * witnesses apart.
     */
    fun deviceName(context: Context): String {
        val stored = prefs.getString("device", null)
        if (!stored.isNullOrBlank()) return stored
        val id = runCatching {
            Secure.getString(context.contentResolver, Secure.ANDROID_ID)
        }.getOrNull()?.take(6) ?: "unknown"
        val name = "${android.os.Build.MODEL.replace(' ', '-')}-$id"
        prefs.edit().putString("device", name).apply()
        return name
    }

    fun configured(): Boolean = baseUrl.isNotBlank()

    fun anySensorEnabled(): Boolean = steps || location || audio || battery
}

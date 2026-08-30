package dev.physis.monitor

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import androidx.core.content.ContextCompat

/**
 * Restarts monitoring after a reboot — but only if it was running when the
 * phone went down, and only with the sensors the user had chosen.
 *
 * A monitor that silently dies at every reboot records a life with holes in it
 * exactly where the phone ran out of battery. A monitor that starts itself
 * after an install is the thing people are right to distrust. The stored
 * `running` flag is the difference.
 */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != Intent.ACTION_BOOT_COMPLETED) return
        val settings = Settings(context)
        if (!settings.running || !settings.configured() || !settings.anySensorEnabled()) return
        ContextCompat.startForegroundService(context, Intent(context, MonitorService::class.java))
    }
}

package dev.physis.monitor

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager
import android.location.Location
import android.location.LocationListener
import android.location.LocationManager
import android.os.BatteryManager
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import org.json.JSONObject
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone
import java.util.concurrent.CopyOnWriteArrayList
import kotlin.concurrent.thread

/**
 * The collector: sensors in, batched samples out to the engine.
 *
 * A foreground service and not a background job, because the honest version of
 * this app is the one the system shows you is running. Android 14+ puts a
 * microphone and location indicator in the status bar while this works; the
 * notification says why it is there rather than trying to be forgettable.
 *
 * It infers nothing. Steps are steps and a fix is a fix; whether that was a
 * workout at the gym is decided by `soma` and `topos` on the engine side, which
 * is the split the whole system is built on — the device perceives, the engine
 * means.
 */
class MonitorService : Service(), SensorEventListener, LocationListener {

    companion object {
        const val CHANNEL_ID = "physis-monitor"
        const val NOTIFICATION_ID = 1
        /** How often a batch is pushed. Long enough that the radio sleeps
         *  between syncs, short enough that a lost phone loses one window. */
        const val SYNC_MINUTES = 15L
        /** Listening windows: 20 s of analysis every 3 minutes. Continuous
         *  capture would cost battery for no extra fact — a conversation does
         *  not need to be heard end to end to be known to have happened. */
        const val AUDIO_WINDOW_SECONDS = 20
        const val AUDIO_EVERY_MINUTES = 3L
    }

    private lateinit var settings: Settings
    private val pending = CopyOnWriteArrayList<JSONObject>()
    @Volatile private var running = false

    private var sensorManager: SensorManager? = null
    private var locationManager: LocationManager? = null

    /** TYPE_STEP_COUNTER reports steps since boot, so the delta between reads
     *  is the interval's count. The first read establishes the baseline and
     *  reports nothing — a fresh install must not claim a day's steps as one
     *  window's workout. */
    private var lastStepTotal: Float? = null
    private var lastStepAt: Long = 0

    private val batteryReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: Intent?) {
            val level = intent?.getIntExtra(BatteryManager.EXTRA_LEVEL, -1) ?: -1
            val scale = intent?.getIntExtra(BatteryManager.EXTRA_SCALE, -1) ?: -1
            if (level < 0 || scale <= 0) return
            val pct = level * 100f / scale
            queue(
                JSONObject()
                    .put("kind", "reading")
                    .put("at", nowIso())
                    .put("metric", "battery")
                    .put("value", pct)
                    // The engine reads value against baseline; 80 is "a normal
                    // day's charge", so a deep discharge reads as the anomaly.
                    .put("baseline", 80.0)
            )
        }
    }

    override fun onCreate() {
        super.onCreate()
        settings = Settings(this)
        createChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        startForeground(NOTIFICATION_ID, notification("watching your day"))
        if (running) return START_STICKY
        running = true
        settings.running = true

        startSensors()
        startSyncLoop()
        if (settings.audio) startAudioLoop()
        return START_STICKY
    }

    override fun onDestroy() {
        running = false
        settings.running = false
        sensorManager?.unregisterListener(this)
        runCatching { locationManager?.removeUpdates(this) }
        runCatching { unregisterReceiver(batteryReceiver) }
        // One last attempt so a deliberate stop does not throw away the window
        // the user was in the middle of.
        thread { sync() }
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    // ── Sensors ───────────────────────────────────────────────────────────

    private fun startSensors() {
        if (settings.steps && granted(android.Manifest.permission.ACTIVITY_RECOGNITION)) {
            sensorManager = getSystemService(SENSOR_SERVICE) as SensorManager
            val counter = sensorManager?.getDefaultSensor(Sensor.TYPE_STEP_COUNTER)
            if (counter != null) {
                // SENSOR_DELAY_NORMAL on a hardware counter: the chip is already
                // counting whether we listen or not, so this costs almost
                // nothing next to polling the accelerometer ourselves.
                sensorManager?.registerListener(this, counter, SensorManager.SENSOR_DELAY_NORMAL)
            }
        }

        if (settings.location && granted(android.Manifest.permission.ACCESS_FINE_LOCATION)) {
            locationManager = getSystemService(LOCATION_SERVICE) as LocationManager
            runCatching {
                // LocationManager, not the fused provider: no Play Services
                // anywhere in this app. Five minutes and 50 m is enough to find
                // the places someone actually spends time in, and far less than
                // a trace.
                locationManager?.requestLocationUpdates(
                    LocationManager.GPS_PROVIDER,
                    5 * 60 * 1000L,
                    50f,
                    this,
                )
            }
        }

        if (settings.battery) {
            ContextCompat.registerReceiver(
                this,
                batteryReceiver,
                IntentFilter(Intent.ACTION_BATTERY_CHANGED),
                ContextCompat.RECEIVER_NOT_EXPORTED,
            )
        }
    }

    override fun onSensorChanged(event: SensorEvent?) {
        val e = event ?: return
        if (e.sensor.type != Sensor.TYPE_STEP_COUNTER) return
        val total = e.values.firstOrNull() ?: return
        val now = System.currentTimeMillis()

        val previous = lastStepTotal
        lastStepTotal = total
        if (previous == null) {
            lastStepAt = now
            return // baseline only; a first read is not a window
        }

        val delta = (total - previous).toInt()
        val minutes = ((now - lastStepAt) / 60_000L).toInt().coerceAtLeast(1)
        lastStepAt = now
        if (delta <= 0) return

        queue(
            JSONObject()
                .put("kind", "steps")
                .put("at", nowIso())
                .put("count", delta)
                .put("minutes", minutes)
        )
    }

    override fun onAccuracyChanged(sensor: Sensor?, accuracy: Int) = Unit

    override fun onLocationChanged(location: Location) {
        queue(
            JSONObject()
                .put("kind", "location")
                .put("at", nowIso())
                .put("lat", location.latitude)
                .put("lng", location.longitude)
                // Sent so the engine can throw the fix away: a place invented
                // from 900 m of error is a place you never went.
                .put("accuracy_m", if (location.hasAccuracy()) location.accuracy else JSONObject.NULL)
        )
    }

    // ── Audio: whether, never what ────────────────────────────────────────

    private fun startAudioLoop() = thread(isDaemon = true) {
        val probe = AudioProbe()
        while (running) {
            if (settings.audio && granted(android.Manifest.permission.RECORD_AUDIO)) {
                val obs = probe.listen(AUDIO_WINDOW_SECONDS)
                // Only a window with speech in it is an event. Silence is not a
                // fact worth carrying, and sending it every three minutes would
                // fill the graph with the absence of things.
                if (obs != null && obs.speechSeconds > 3) {
                    queue(
                        JSONObject()
                            .put("kind", "audio")
                            .put("at", nowIso())
                            // Scaled to the window it represents: 20 s of
                            // listening stands for the 3 minutes around it.
                            .put("minutes", AUDIO_EVERY_MINUTES.toInt())
                            .put("voices", obs.voices ?: JSONObject.NULL)
                    )
                }
            }
            sleepMinutes(AUDIO_EVERY_MINUTES)
        }
    }

    // ── Sync ──────────────────────────────────────────────────────────────

    private fun startSyncLoop() = thread(isDaemon = true) {
        while (running) {
            sleepMinutes(SYNC_MINUTES)
            sync()
        }
    }

    private fun queue(sample: JSONObject) {
        pending.add(sample)
        // A phone that cannot reach its engine for a day should not grow without
        // bound. The oldest go first: the recent past is the part still worth
        // knowing, and the alternative is an OOM that loses everything.
        while (pending.size > 4_000) pending.removeAt(0)
    }

    private fun sync() {
        if (pending.isEmpty() || !settings.configured()) return
        val batch = pending.toList()
        val client = PhysisClient(settings.baseUrl, settings.token, settings.user)
        val result = client.send(settings.deviceName(this), batch)
        if (result.ok) {
            // Only what was actually sent is dropped; anything queued during the
            // request stays for the next round.
            pending.removeAll(batch.toSet())
            settings.lastSync = "${stamp()} · ${result.detail}"
            update("synced ${batch.size} samples")
        } else {
            // Keep them. The engine's translation is pure and registration
            // dedupes by label, so replaying after a dropped connection is safe.
            settings.lastSync = "${stamp()} · failed: ${result.detail}"
            update("offline · ${pending.size} samples waiting")
        }
    }

    // ── Notification ──────────────────────────────────────────────────────

    private fun createChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val channel = NotificationChannel(
            CHANNEL_ID,
            "Monitoring",
            NotificationManager.IMPORTANCE_LOW,
        ).apply {
            description = "Shown while Physis is watching your day"
            setShowBadge(false)
        }
        (getSystemService(NOTIFICATION_SERVICE) as NotificationManager)
            .createNotificationChannel(channel)
    }

    private fun notification(text: String): Notification {
        val open = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE,
        )
        val sensors = buildList {
            if (settings.steps) add("steps")
            if (settings.location) add("places")
            if (settings.audio) add("speech (no audio kept)")
            if (settings.battery) add("battery")
        }.joinToString(", ").ifEmpty { "nothing selected" }

        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("Physis · $text")
            // The status-bar dot is going to be there; this says why, which is
            // the difference between a monitor someone chose and one they
            // discover.
            .setContentText("Recording: $sensors")
            .setSmallIcon(android.R.drawable.ic_menu_compass)
            .setOngoing(true)
            .setContentIntent(open)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .build()
    }

    private fun update(text: String) {
        (getSystemService(NOTIFICATION_SERVICE) as NotificationManager)
            .notify(NOTIFICATION_ID, notification(text))
    }

    // ── Small helpers ─────────────────────────────────────────────────────

    private fun granted(permission: String): Boolean =
        ContextCompat.checkSelfPermission(this, permission) == PackageManager.PERMISSION_GRANTED

    private fun sleepMinutes(minutes: Long) {
        // Interruptible in small steps so stopping the service does not wait out
        // a fifteen-minute sleep.
        val until = System.currentTimeMillis() + minutes * 60_000
        while (running && System.currentTimeMillis() < until) {
            runCatching { Thread.sleep(5_000) }
        }
    }

    private fun nowIso(): String {
        val fmt = SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss'Z'", Locale.US)
        fmt.timeZone = TimeZone.getTimeZone("UTC")
        return fmt.format(Date())
    }

    private fun stamp(): String =
        SimpleDateFormat("HH:mm", Locale.getDefault()).format(Date())
}

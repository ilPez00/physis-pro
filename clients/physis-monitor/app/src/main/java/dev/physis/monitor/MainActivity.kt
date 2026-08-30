package dev.physis.monitor

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.text.InputType
import android.view.Gravity
import android.view.ViewGroup
import android.widget.Button
import android.widget.CheckBox
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat

/**
 * One screen: where to sync, what to record, and — in plain words — what that
 * means. Built in code rather than XML because it is a settings form, and a
 * layout file would be a second place to keep the same six controls.
 *
 * Every sensor is off until switched on, and each switch asks for its own
 * permission at the moment it is switched. An app that requests microphone
 * access on first launch has already told the user what kind of app it is.
 */
class MainActivity : AppCompatActivity() {

    private lateinit var settings: Settings
    private lateinit var status: TextView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        settings = Settings(this)

        val pad = (16 * resources.displayMetrics.density).toInt()
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(pad, pad, pad, pad)
        }

        fun heading(text: String) = TextView(this).apply {
            this.text = text
            textSize = 13f
            setPadding(0, pad, 0, pad / 3)
            alpha = 0.7f
        }

        fun field(label: String, value: String, password: Boolean = false): EditText {
            root.addView(heading(label))
            return EditText(this).apply {
                setText(value)
                if (password) inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
                root.addView(this, ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)
            }
        }

        TextView(this).apply {
            text = "Physis Monitor"
            textSize = 22f
            gravity = Gravity.START
            root.addView(this)
        }
        TextView(this).apply {
            // The honest pitch, on the first screen, before any permission is
            // requested: what leaves the phone, and what never does.
            text = "Your phone notices things you never write down: that there " +
                "was a workout, that it happened at the gym, that the Tuesday " +
                "lesson did not happen this week.\n\n" +
                "This app sends those observations to YOUR OWN Physis engine — " +
                "not to us, not to anyone else. It never records audio: it " +
                "measures whether someone was speaking and roughly how many " +
                "people, then throws the sound away in the same breath."
            textSize = 13f
            setPadding(0, pad / 2, 0, pad / 2)
            root.addView(this)
        }

        val urlField = field("Engine address (e.g. http://10.0.0.5:19876)", settings.baseUrl)
        val tokenField = field("API token (optional)", settings.token, password = true)
        val userField = field("Sync as (X-Physis-User, optional)", settings.user)

        root.addView(heading("What to record — each asks permission when you turn it on"))

        val steps = checkbox("Steps — walks and workouts", settings.steps) { on ->
            if (on) request(stepPermission())
            settings.steps = on
        }
        val location = checkbox("Places — where you spend time", settings.location) { on ->
            if (on) request(arrayOf(Manifest.permission.ACCESS_FINE_LOCATION))
            settings.location = on
        }
        val audio = checkbox("Speech — that talking happened, never what was said", settings.audio) { on ->
            if (on) request(arrayOf(Manifest.permission.RECORD_AUDIO))
            settings.audio = on
        }
        val battery = checkbox("Battery — how hard the day was on the phone", settings.battery) { on ->
            settings.battery = on
        }
        listOf(steps, location, audio, battery).forEach { root.addView(it) }

        status = TextView(this).apply {
            textSize = 12f
            alpha = 0.75f
            setPadding(0, pad, 0, pad / 2)
        }

        val start = Button(this).apply {
            text = "Start monitoring"
            setOnClickListener {
                settings.baseUrl = urlField.text.toString()
                settings.token = tokenField.text.toString()
                settings.user = userField.text.toString()
                if (!settings.configured()) {
                    toast("Set your engine address first")
                    return@setOnClickListener
                }
                if (!settings.anySensorEnabled()) {
                    toast("Choose at least one thing to record")
                    return@setOnClickListener
                }
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                    request(arrayOf(Manifest.permission.POST_NOTIFICATIONS))
                }
                ContextCompat.startForegroundService(
                    this@MainActivity,
                    Intent(this@MainActivity, MonitorService::class.java),
                )
                refresh()
            }
        }
        val stop = Button(this).apply {
            text = "Stop"
            setOnClickListener {
                stopService(Intent(this@MainActivity, MonitorService::class.java))
                refresh()
            }
        }
        root.addView(start)
        root.addView(stop)
        root.addView(status)

        TextView(this).apply {
            text = "While monitoring runs, Android shows a microphone or location " +
                "dot in your status bar. That dot is correct — it is there because " +
                "you asked this app to notice your day, and it goes out when you stop."
            textSize = 12f
            alpha = 0.6f
            setPadding(0, pad, 0, pad)
            root.addView(this)
        }

        setContentView(ScrollView(this).apply { addView(root) })
        refresh()
    }

    override fun onResume() {
        super.onResume()
        refresh()
    }

    private fun refresh() {
        val running = settings.running
        status.text = buildString {
            append(if (running) "Monitoring is on.\n" else "Monitoring is off.\n")
            append("Device: ${settings.deviceName(this@MainActivity)}\n")
            append("Last sync: ${settings.lastSync}")
            if (settings.location && !granted(Manifest.permission.ACCESS_BACKGROUND_LOCATION)) {
                // Background location cannot be requested in a dialog on modern
                // Android; the user has to choose "Allow all the time" in
                // settings. Saying so beats silently recording nothing once the
                // screen goes off.
                append("\n\nPlaces will only be recorded while the app is open. " +
                    "To record them all day, set Location to \"Allow all the time\" " +
                    "in Android settings.")
            }
        }
    }

    private fun checkbox(label: String, checked: Boolean, onChange: (Boolean) -> Unit) =
        CheckBox(this).apply {
            text = label
            isChecked = checked
            setOnCheckedChangeListener { _, on -> onChange(on) }
        }

    private fun stepPermission(): Array<String> =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            arrayOf(Manifest.permission.ACTIVITY_RECOGNITION)
        } else {
            emptyArray()
        }

    private fun request(permissions: Array<String>) {
        val missing = permissions.filterNot { granted(it) }
        if (missing.isNotEmpty()) {
            requestPermissions(missing.toTypedArray(), 1)
        }
    }

    private fun granted(permission: String): Boolean =
        ContextCompat.checkSelfPermission(this, permission) == PackageManager.PERMISSION_GRANTED

    private fun toast(text: String) =
        Toast.makeText(this, text, Toast.LENGTH_SHORT).show()
}

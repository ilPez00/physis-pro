package dev.physis.monitor

import org.json.JSONArray
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.URL

/**
 * Posts a batch to the engine's `/api/v1/monitor/ingest`.
 *
 * `HttpURLConnection` and `org.json` rather than OkHttp and Moshi: both are in
 * AOSP, so the APK gains no dependency a reviewer has to audit and no licence
 * anyone has to check. The payload is small and infrequent; a better HTTP
 * client would buy nothing.
 */
class PhysisClient(
    private val baseUrl: String,
    private val token: String,
    private val user: String,
) {

    data class Result(val ok: Boolean, val detail: String)

    /**
     * Send [samples]. Returns what the engine said it understood, so the app can
     * show the user the truth rather than a checkmark.
     *
     * A failed sync keeps its samples: the caller re-queues them. The engine's
     * translation is pure and its registration dedupes by label, so replaying a
     * batch after a dropped connection is safe — which is why retrying is the
     * right behaviour rather than a risk of double-counting the morning.
     */
    fun send(device: String, samples: List<JSONObject>): Result {
        val body = JSONObject()
            .put("device", device)
            .put("samples", JSONArray(samples))
            .toString()

        val url = URL(baseUrl.trimEnd('/') + "/api/v1/monitor/ingest")
        val conn = url.openConnection() as HttpURLConnection
        return try {
            conn.requestMethod = "POST"
            conn.doOutput = true
            conn.connectTimeout = 15_000
            conn.readTimeout = 30_000
            conn.setRequestProperty("Content-Type", "application/json")
            if (token.isNotBlank()) {
                conn.setRequestProperty("Authorization", "Bearer $token")
            }
            if (user.isNotBlank()) {
                conn.setRequestProperty("X-Physis-User", user)
            }
            conn.outputStream.use { it.write(body.toByteArray()) }

            val code = conn.responseCode
            val text = (if (code in 200..299) conn.inputStream else conn.errorStream)
                ?.bufferedReader()?.use { it.readText() }.orEmpty()

            if (code in 200..299) {
                val json = runCatching { JSONObject(text) }.getOrNull()
                val understood = json?.optJSONObject("understood")
                val registered = json?.optInt("registered") ?: 0
                Result(
                    ok = true,
                    detail = "registered $registered · " +
                        "${understood?.optInt("events") ?: 0} events, " +
                        "${understood?.optInt("sessions") ?: 0} sessions, " +
                        "${understood?.optInt("waypoints") ?: 0} places",
                )
            } else {
                Result(false, "HTTP $code: ${text.take(180)}")
            }
        } catch (e: Exception) {
            // The device is often offline; that is normal, not an error state
            // worth alarming about. The caller keeps the samples and tries later.
            Result(false, e.message ?: e.javaClass.simpleName)
        } finally {
            conn.disconnect()
        }
    }
}

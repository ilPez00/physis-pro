package dev.physis.monitor

import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import kotlin.math.abs
import kotlin.math.sqrt

/**
 * Listens for *whether* speech is happening, never to what is said.
 *
 * Recording yourself is your business. A lesson or a call captures other
 * people, and in Italy that engages art. 615-bis / 617 c.p. and leaves the
 * GDPR household exemption the moment it is stored. The liability lives in
 * keeping the audio; the value lives in the inference, and the inference does
 * not need it.
 *
 * So this class is built so that keeping audio is not a policy someone has to
 * remember — it is not reachable from here:
 *
 *  * one [ShortArray] is allocated once and overwritten on every read;
 *  * no `File`, `OutputStream`, `Uri` or network call appears in this file;
 *  * [Observation] carries three numbers and a voice estimate, no samples;
 *  * the recorder is released before the observation is returned.
 *
 * What survives a session is "someone spoke for 50 minutes, probably more than
 * one person" — the shape of the day, which is what the engine wanted.
 */
class AudioProbe {

    /**
     * What one listening window concluded. Numbers only.
     *
     * @param speechSeconds how much of the window contained speech-like sound
     * @param voices best guess at how many people were talking, or null when
     *   the window gave no honest basis for a guess. Null is a real answer and
     *   is reported as such rather than defaulting to 1.
     */
    data class Observation(
        val speechSeconds: Int,
        val voices: Int?,
    )

    /** 16 kHz mono is ample for deciding "is this speech"; it is far too coarse
     *  to be a pleasant recording, which is a property rather than a cost. */
    private val sampleRate = 16_000

    /**
     * Listen for [windowSeconds] and return what was inferred.
     *
     * Returns null when the microphone could not be opened at all — a denied
     * permission or another app holding it. A silent null beats inventing a
     * quiet window that never happened.
     */
    fun listen(windowSeconds: Int): Observation? {
        val minBuf = AudioRecord.getMinBufferSize(
            sampleRate,
            AudioFormat.CHANNEL_IN_MONO,
            AudioFormat.ENCODING_PCM_16BIT,
        )
        if (minBuf <= 0) return null

        val recorder = try {
            AudioRecord(
                MediaRecorder.AudioSource.MIC,
                sampleRate,
                AudioFormat.CHANNEL_IN_MONO,
                AudioFormat.ENCODING_PCM_16BIT,
                minBuf * 2,
            )
        } catch (e: SecurityException) {
            return null
        }
        if (recorder.state != AudioRecord.STATE_INITIALIZED) {
            recorder.release()
            return null
        }

        // The only buffer that will ever exist. Every read overwrites the
        // previous frame, so at no point does the process hold more than ~64 ms
        // of sound, and nothing outlives this function.
        val frame = ShortArray(sampleRate / 16) // ~62 ms
        val framesPerSecond = 16
        val totalFrames = windowSeconds * framesPerSecond

        var speechFrames = 0
        var turns = 0                 // silence → speech transitions
        var wasSpeech = false
        var quietFrames = 0
        // Noise floor adapts: a library and a workshop have different silences,
        // and a fixed threshold would call one of them speech all day.
        var floor = 0.0

        try {
            recorder.startRecording()
            repeat(totalFrames) {
                val read = recorder.read(frame, 0, frame.size)
                if (read <= 0) return@repeat

                val rms = rms(frame, read)
                val zcr = zeroCrossingRate(frame, read)

                floor = if (floor == 0.0) rms else (floor * 0.95 + rms * 0.05)

                // Speech is louder than the running floor *and* has the
                // zero-crossing range of voice. Loudness alone calls a slammed
                // door speech; the ZCR band rejects most of that.
                val isSpeech = rms > floor * 2.2 && zcr in 0.02..0.35

                if (isSpeech) {
                    speechFrames++
                    // A new turn only after real silence, so one person pausing
                    // mid-sentence is not counted as a second speaker.
                    if (!wasSpeech && quietFrames >= framesPerSecond) turns++
                    wasSpeech = true
                    quietFrames = 0
                } else {
                    quietFrames++
                    if (quietFrames >= framesPerSecond / 2) wasSpeech = false
                }
            }
        } catch (e: IllegalStateException) {
            return null
        } finally {
            // Released before anything is returned: the microphone is not held
            // while the result is handed on, and the indicator in the status bar
            // goes out when listening stops rather than when the app feels like it.
            try { recorder.stop() } catch (_: IllegalStateException) {}
            recorder.release()
        }

        val speechSeconds = speechFrames / framesPerSecond
        if (speechSeconds == 0) return Observation(0, null)

        // Voice counting from turn-taking is a *coarse* signal and is reported
        // as one: several separated turns means a conversation, one continuous
        // stretch means one voice, and anything in between is left null rather
        // than guessed. A wrong count is worse than an absent one, because the
        // graph would then hold "you met someone" on no evidence.
        val voices = when {
            turns >= 4 -> 2
            turns <= 1 && speechSeconds > 5 -> 1
            else -> null
        }
        return Observation(speechSeconds, voices)
    }

    private fun rms(buf: ShortArray, n: Int): Double {
        var sum = 0.0
        for (i in 0 until n) {
            val v = buf[i].toDouble()
            sum += v * v
        }
        return sqrt(sum / n) / Short.MAX_VALUE
    }

    private fun zeroCrossingRate(buf: ShortArray, n: Int): Double {
        if (n < 2) return 0.0
        var crossings = 0
        for (i in 1 until n) {
            if ((buf[i] >= 0) != (buf[i - 1] >= 0)) crossings++
        }
        return crossings.toDouble() / n
    }
}

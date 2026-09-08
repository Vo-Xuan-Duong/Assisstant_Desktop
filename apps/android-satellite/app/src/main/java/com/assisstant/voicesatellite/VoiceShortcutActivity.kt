package com.assisstant.voicesatellite

import android.app.Activity
import android.content.Intent
import android.os.Bundle

/**
 * Minimal exported trampoline used only by Android's static launcher shortcut.
 * MainActivity remains authoritative for pairing, microphone permission,
 * connection readiness, cancellation and SpeechRecognizer lifecycle.
 */
class VoiceShortcutActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val voiceIntent = Intent(this, MainActivity::class.java).apply {
            addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP)
            putExtra(MainActivity.EXTRA_START_VOICE, true)
        }
        startActivity(voiceIntent)
        finish()
    }
}

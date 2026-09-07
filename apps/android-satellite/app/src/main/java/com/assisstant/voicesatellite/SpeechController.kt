package com.assisstant.voicesatellite

import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer

class SpeechController(
    private val context: Context,
    private val onListeningChanged: (Boolean) -> Unit,
    private val onPartialText: (String) -> Unit,
    private val onFinalText: (String) -> Unit,
    private val onError: (String) -> Unit,
) : RecognitionListener {
    private var recognizer: SpeechRecognizer? = null
    private var usingOnDevice = false

    fun start(languageTag: String, preferOnDevice: Boolean) {
        if (!SpeechRecognizer.isRecognitionAvailable(context)) {
            onError("Thiết bị không có dịch vụ nhận dạng giọng nói.")
            return
        }

        val shouldUseOnDevice = preferOnDevice &&
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.S &&
            SpeechRecognizer.isOnDeviceRecognitionAvailable(context)

        if (recognizer == null || shouldUseOnDevice != usingOnDevice) {
            recreateRecognizer(shouldUseOnDevice)
        }

        val intent = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
            putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
            putExtra(RecognizerIntent.EXTRA_LANGUAGE, languageTag)
            putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, true)
            putExtra(RecognizerIntent.EXTRA_MAX_RESULTS, 3)
            if (shouldUseOnDevice) {
                putExtra(RecognizerIntent.EXTRA_PREFER_OFFLINE, true)
            }
        }

        onPartialText("")
        onListeningChanged(true)
        try {
            recognizer?.startListening(intent)
        } catch (error: Exception) {
            onListeningChanged(false)
            onError(error.message ?: "Không thể bắt đầu nhận dạng giọng nói.")
        }
    }

    fun stop() {
        runCatching { recognizer?.stopListening() }
    }

    fun cancel() {
        runCatching { recognizer?.cancel() }
        onListeningChanged(false)
    }

    fun destroy() {
        recognizer?.destroy()
        recognizer = null
        onListeningChanged(false)
    }

    private fun recreateRecognizer(onDevice: Boolean) {
        recognizer?.destroy()
        recognizer = if (onDevice && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            SpeechRecognizer.createOnDeviceSpeechRecognizer(context)
        } else {
            SpeechRecognizer.createSpeechRecognizer(context)
        }
        usingOnDevice = onDevice
        recognizer?.setRecognitionListener(this)
    }

    override fun onReadyForSpeech(params: Bundle?) {
        onListeningChanged(true)
    }

    override fun onBeginningOfSpeech() {
        onListeningChanged(true)
    }

    override fun onRmsChanged(rmsdB: Float) = Unit

    override fun onBufferReceived(buffer: ByteArray?) = Unit

    override fun onEndOfSpeech() {
        onListeningChanged(false)
    }

    override fun onError(error: Int) {
        onListeningChanged(false)
        val message = when (error) {
            SpeechRecognizer.ERROR_AUDIO -> "Lỗi microphone/audio."
            SpeechRecognizer.ERROR_CLIENT -> "Phiên nhận dạng đã bị hủy."
            SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS -> "Ứng dụng chưa có quyền microphone."
            SpeechRecognizer.ERROR_NETWORK, SpeechRecognizer.ERROR_NETWORK_TIMEOUT -> "Dịch vụ nhận dạng đang gặp lỗi mạng."
            SpeechRecognizer.ERROR_NO_MATCH -> "Chưa nhận ra câu nói. Hãy thử lại."
            SpeechRecognizer.ERROR_RECOGNIZER_BUSY -> "Bộ nhận dạng đang bận."
            SpeechRecognizer.ERROR_SERVER, SpeechRecognizer.ERROR_SERVER_DISCONNECTED -> "Dịch vụ nhận dạng giọng nói không sẵn sàng."
            SpeechRecognizer.ERROR_SPEECH_TIMEOUT -> "Không phát hiện giọng nói."
            else -> "Nhận dạng giọng nói thất bại (mã $error)."
        }
        if (error != SpeechRecognizer.ERROR_CLIENT) {
            onError(message)
        }
    }

    override fun onResults(results: Bundle?) {
        onListeningChanged(false)
        val best = results
            ?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)
            ?.firstOrNull()
            ?.trim()
            .orEmpty()
        if (best.isNotEmpty()) {
            onPartialText(best)
            onFinalText(best)
        }
    }

    override fun onPartialResults(partialResults: Bundle?) {
        val best = partialResults
            ?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)
            ?.firstOrNull()
            ?.trim()
            .orEmpty()
        if (best.isNotEmpty()) {
            onPartialText(best)
        }
    }

    override fun onEvent(eventType: Int, params: Bundle?) = Unit
}

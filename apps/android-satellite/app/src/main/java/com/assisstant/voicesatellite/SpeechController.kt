package com.assisstant.voicesatellite

import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer

data class SpeechRecognitionResult(
    val text: String,
    val alternatives: List<String>,
    val confidence: Float?,
    val elapsedMs: Long,
    val usedOnDeviceRecognizer: Boolean,
)

class SpeechController(
    private val context: Context,
    private val onListeningChanged: (Boolean) -> Unit,
    private val onPartialText: (String) -> Unit,
    private val onFinalResult: (SpeechRecognitionResult) -> Unit,
    private val onStatus: (String) -> Unit,
    private val onError: (String) -> Unit,
) : RecognitionListener {
    companion object {
        private const val RETRY_DELAY_MS = 350L
        private const val MAX_RESULT_CANDIDATES = 3
    }

    private val mainHandler = Handler(Looper.getMainLooper())
    private var recognizer: SpeechRecognizer? = null
    private var usingOnDevice = false
    private var lastLanguageTag = "vi-VN"
    private var requestedPreferOnDevice = true
    private var fallbackAttempted = false
    private var busyRetryAttempted = false
    private var intentionallyCancelled = false
    private var requestStartedAtMs = 0L

    fun start(languageTag: String, preferOnDevice: Boolean) {
        if (!SpeechRecognizer.isRecognitionAvailable(context)) {
            onError("Thiết bị không có dịch vụ nhận dạng giọng nói.")
            return
        }

        lastLanguageTag = languageTag
        requestedPreferOnDevice = preferOnDevice
        fallbackAttempted = false
        busyRetryAttempted = false
        intentionallyCancelled = false
        requestStartedAtMs = SystemClock.elapsedRealtime()

        val shouldUseOnDevice = preferOnDevice &&
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.S &&
            SpeechRecognizer.isOnDeviceRecognitionAvailable(context)
        startInternal(languageTag, shouldUseOnDevice)
    }

    fun stop() {
        runCatching { recognizer?.stopListening() }
    }

    fun cancel() {
        intentionallyCancelled = true
        mainHandler.removeCallbacksAndMessages(null)
        runCatching { recognizer?.cancel() }
        onListeningChanged(false)
    }

    fun destroy() {
        intentionallyCancelled = true
        mainHandler.removeCallbacksAndMessages(null)
        recognizer?.destroy()
        recognizer = null
        onListeningChanged(false)
    }

    private fun startInternal(languageTag: String, onDevice: Boolean) {
        if (intentionallyCancelled) return
        if (recognizer == null || onDevice != usingOnDevice) {
            recreateRecognizer(onDevice)
        }

        val intent = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
            putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
            putExtra(RecognizerIntent.EXTRA_LANGUAGE, languageTag)
            putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, true)
            putExtra(RecognizerIntent.EXTRA_MAX_RESULTS, MAX_RESULT_CANDIDATES)
            if (onDevice) {
                putExtra(RecognizerIntent.EXTRA_PREFER_OFFLINE, true)
            }
        }

        onPartialText("")
        onListeningChanged(true)
        try {
            recognizer?.startListening(intent)
        } catch (error: Exception) {
            onListeningChanged(false)
            if (!tryRecoverFromStartFailure(onDevice)) {
                onError(error.message ?: "Không thể bắt đầu nhận dạng giọng nói.")
            }
        }
    }

    private fun tryRecoverFromStartFailure(onDevice: Boolean): Boolean {
        if (intentionallyCancelled) return false
        if (onDevice && !fallbackAttempted) {
            fallbackAttempted = true
            onStatus("Bộ nhận dạng on-device chưa sẵn sàng; đang chuyển sang SpeechRecognizer hệ thống.")
            recreateRecognizer(false)
            mainHandler.postDelayed({ startInternal(lastLanguageTag, false) }, RETRY_DELAY_MS)
            return true
        }
        if (!busyRetryAttempted) {
            busyRetryAttempted = true
            onStatus("Bộ nhận dạng đang bận; đang thử khởi tạo lại một lần.")
            recreateRecognizer(onDevice)
            mainHandler.postDelayed({ startInternal(lastLanguageTag, onDevice) }, RETRY_DELAY_MS)
            return true
        }
        return false
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
        if (intentionallyCancelled) return

        val engineFailure = error == SpeechRecognizer.ERROR_SERVER ||
            error == SpeechRecognizer.ERROR_SERVER_DISCONNECTED ||
            error == SpeechRecognizer.ERROR_RECOGNIZER_BUSY

        if (usingOnDevice && engineFailure && requestedPreferOnDevice && !fallbackAttempted) {
            fallbackAttempted = true
            onStatus("Nhận dạng on-device tạm thời không sẵn sàng; đang dùng dịch vụ nhận dạng hệ thống.")
            recreateRecognizer(false)
            mainHandler.postDelayed({ startInternal(lastLanguageTag, false) }, RETRY_DELAY_MS)
            return
        }

        if (error == SpeechRecognizer.ERROR_RECOGNIZER_BUSY && !busyRetryAttempted) {
            busyRetryAttempted = true
            val retryOnDevice = usingOnDevice
            onStatus("Bộ nhận dạng đang bận; đang thử khởi tạo lại một lần.")
            recreateRecognizer(retryOnDevice)
            mainHandler.postDelayed({ startInternal(lastLanguageTag, retryOnDevice) }, RETRY_DELAY_MS)
            return
        }

        val message = when (error) {
            SpeechRecognizer.ERROR_AUDIO -> "Lỗi microphone/audio."
            SpeechRecognizer.ERROR_CLIENT -> "Phiên nhận dạng kết thúc do lỗi phía ứng dụng."
            SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS -> "Ứng dụng chưa có quyền microphone."
            SpeechRecognizer.ERROR_NETWORK, SpeechRecognizer.ERROR_NETWORK_TIMEOUT -> "Dịch vụ nhận dạng đang gặp lỗi mạng."
            SpeechRecognizer.ERROR_NO_MATCH -> "Chưa nhận ra câu nói. Hãy thử lại."
            SpeechRecognizer.ERROR_RECOGNIZER_BUSY -> "Bộ nhận dạng đang bận. Hãy thử lại."
            SpeechRecognizer.ERROR_SERVER, SpeechRecognizer.ERROR_SERVER_DISCONNECTED -> "Dịch vụ nhận dạng giọng nói không sẵn sàng."
            SpeechRecognizer.ERROR_SPEECH_TIMEOUT -> "Không phát hiện giọng nói."
            else -> "Nhận dạng giọng nói thất bại (mã $error)."
        }
        onError(message)
    }

    override fun onResults(results: Bundle?) {
        onListeningChanged(false)
        val candidates = results
            ?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)
            .orEmpty()
            .map(String::trim)
            .filter(String::isNotEmpty)
            .distinct()
            .take(MAX_RESULT_CANDIDATES)

        val best = candidates.firstOrNull().orEmpty()
        if (best.isEmpty()) {
            onError("Không nhận được nội dung giọng nói cuối cùng. Hãy thử lại.")
            return
        }

        val confidence = results
            ?.getFloatArray(SpeechRecognizer.CONFIDENCE_SCORES)
            ?.getOrNull(0)
            ?.takeIf { score -> score in 0.0f..1.0f }
        val elapsedMs = (SystemClock.elapsedRealtime() - requestStartedAtMs).coerceAtLeast(0L)

        onPartialText(best)
        onFinalResult(
            SpeechRecognitionResult(
                text = best,
                alternatives = candidates.drop(1),
                confidence = confidence,
                elapsedMs = elapsedMs,
                usedOnDeviceRecognizer = usingOnDevice,
            )
        )
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

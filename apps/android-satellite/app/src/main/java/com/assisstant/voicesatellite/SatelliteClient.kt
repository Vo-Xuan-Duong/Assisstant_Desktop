package com.assisstant.voicesatellite

import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.Looper
import org.json.JSONObject
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import java.util.UUID
import java.util.concurrent.TimeUnit
import kotlin.math.min

class SatelliteClient(
    context: Context,
    private val onConnectionChanged: (Boolean, String) -> Unit,
    private val onTurnState: (String) -> Unit,
    private val onResponse: (String, String?) -> Unit,
    private val onError: (String) -> Unit,
) {
    companion object {
        private const val INITIAL_RECONNECT_DELAY_MS = 1_000L
        private const val MAX_RECONNECT_DELAY_MS = 15_000L
    }

    private val mainHandler = Handler(Looper.getMainLooper())
    private val httpClient = OkHttpClient.Builder()
        .pingInterval(20, TimeUnit.SECONDS)
        .build()
    private val deviceId = loadOrCreateDeviceId(context.applicationContext)
    private val deviceName = "${Build.MANUFACTURER} ${Build.MODEL}".trim()

    @Volatile
    private var webSocket: WebSocket? = null

    @Volatile
    private var ready = false

    @Volatile
    private var connectedUrl: String? = null

    @Volatile
    private var connectedToken: String? = null

    @Volatile
    private var activeCommandId: String? = null

    @Volatile
    private var desiredConnection = false

    @Volatile
    private var connectionGeneration = 0L

    @Volatile
    private var reconnectScheduled = false

    private var reconnectAttempt = 0

    fun connect(rawUrl: String, token: String) {
        val url = normalizeUrl(rawUrl)
        if (url == null) {
            post { onError("Địa chỉ desktop không hợp lệ. Ví dụ: ws://192.168.1.20:8765") }
            return
        }
        val normalizedToken = token.trim()
        if (normalizedToken.length < 16) {
            post { onError("Pairing token phải có ít nhất 16 ký tự.") }
            return
        }

        val generation = connectionGeneration + 1
        connectionGeneration = generation
        desiredConnection = true
        reconnectScheduled = false
        reconnectAttempt = 0
        connectedUrl = url
        connectedToken = normalizedToken
        activeCommandId = null
        ready = false

        webSocket?.cancel()
        webSocket = null
        post { onConnectionChanged(false, "Đang kết nối…") }
        openWebSocket(url, normalizedToken, generation)
    }

    fun isReady(): Boolean = ready

    fun sendCommand(text: String, responseLanguage: String): Boolean {
        val socket = webSocket ?: return false
        if (!ready) return false
        val command = text.trim()
        if (command.isEmpty()) return false

        val commandId = UUID.randomUUID().toString()
        val payload = JSONObject()
            .put("type", "command")
            .put("id", commandId)
            .put("text", command)
            .put("response_language", responseLanguage)
        val sent = socket.send(payload.toString())
        if (sent) {
            activeCommandId = commandId
        }
        return sent
    }

    /**
     * Opens a short-lived authenticated control WebSocket so Stop can be handled
     * while the primary session is blocked waiting for a long Assistant turn.
     */
    fun sendCancel(): Boolean {
        val url = connectedUrl ?: return false
        val token = connectedToken ?: return false
        val commandId = activeCommandId
        val request = runCatching { Request.Builder().url(url).build() }.getOrNull() ?: return false

        httpClient.newWebSocket(request, object : WebSocketListener() {
            private var cancelSent = false

            override fun onOpen(webSocket: WebSocket, response: Response) {
                webSocket.send(helloPayload(token).toString())
            }

            override fun onMessage(webSocket: WebSocket, text: String) {
                val payload = runCatching { JSONObject(text) }.getOrNull() ?: return
                when (payload.optString("type")) {
                    "ready" -> if (!cancelSent) {
                        cancelSent = true
                        val cancel = JSONObject()
                            .put("type", "cancel")
                        if (commandId != null) {
                            cancel.put("id", commandId)
                        }
                        if (!webSocket.send(cancel.toString())) {
                            post { onError("Không gửi được yêu cầu dừng Assistant.") }
                            webSocket.close(1011, "cancel send failed")
                        }
                    }
                    "cancelled" -> {
                        val accepted = payload.optBoolean("accepted", false)
                        if (accepted) {
                            post { onTurnState("cancelled") }
                        } else {
                            post { onError("Desktop hiện không có tác vụ có thể dừng.") }
                        }
                        webSocket.close(1000, "cancel complete")
                    }
                    "error" -> {
                        val message = payload.optString("message", "Desktop từ chối yêu cầu dừng.")
                        post { onError(message) }
                        webSocket.close(1008, "cancel rejected")
                    }
                }
            }

            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                post { onError(t.message ?: "Không thể mở kênh điều khiển để dừng Assistant.") }
            }
        })
        return true
    }

    fun close() {
        desiredConnection = false
        reconnectScheduled = false
        reconnectAttempt = 0
        connectionGeneration += 1
        ready = false
        activeCommandId = null
        connectedUrl = null
        connectedToken = null
        webSocket?.close(1000, "satellite closing")
        webSocket = null
    }

    fun shutdown() {
        close()
        mainHandler.removeCallbacksAndMessages(null)
        httpClient.dispatcher.executorService.shutdown()
        httpClient.connectionPool.evictAll()
    }

    private fun openWebSocket(url: String, token: String, generation: Long) {
        if (!isCurrentConnection(generation)) return
        reconnectScheduled = false
        val request = runCatching { Request.Builder().url(url).build() }.getOrElse { error ->
            post { onError(error.message ?: "Không thể tạo kết nối WebSocket.") }
            scheduleReconnect(url, token, generation)
            return
        }

        val socket = httpClient.newWebSocket(request, object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) {
                if (!isCurrentConnection(generation)) {
                    webSocket.close(1000, "stale connection")
                    return
                }
                webSocket.send(helloPayload(token).toString())
            }

            override fun onMessage(webSocket: WebSocket, text: String) {
                if (!isCurrentConnection(generation)) return
                handleMessage(webSocket, text, generation)
            }

            override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
                webSocket.close(code, reason)
            }

            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                if (!isCurrentConnection(generation)) return
                ready = false
                activeCommandId = null
                this@SatelliteClient.webSocket = null
                post { onConnectionChanged(false, "Đã ngắt kết nối") }
                scheduleReconnect(url, token, generation)
            }

            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                if (!isCurrentConnection(generation)) return
                ready = false
                activeCommandId = null
                this@SatelliteClient.webSocket = null
                post {
                    onConnectionChanged(false, "Mất kết nối")
                    onError(t.message ?: "Kết nối WebSocket thất bại.")
                }
                scheduleReconnect(url, token, generation)
            }
        })
        webSocket = socket
    }

    private fun handleMessage(webSocket: WebSocket, raw: String, generation: Long) {
        val payload = try {
            JSONObject(raw)
        } catch (_: Exception) {
            post { onError("Desktop trả về dữ liệu không hợp lệ.") }
            return
        }

        when (payload.optString("type")) {
            "ready" -> {
                reconnectAttempt = 0
                reconnectScheduled = false
                ready = true
                post { onConnectionChanged(true, "Đã kết nối") }
            }
            "state" -> {
                val state = payload.optString("state", "processing")
                post { onTurnState(state) }
            }
            "response" -> {
                activeCommandId = null
                val text = payload.optString("text")
                val ttsError = payload.optString("tts_error").takeIf { it.isNotBlank() && it != "null" }
                post { onResponse(text, ttsError) }
            }
            "cancelled" -> {
                activeCommandId = null
                post { onTurnState("cancelled") }
            }
            "error" -> {
                val code = payload.optString("code")
                val id = payload.optString("id").takeIf { it.isNotBlank() && it != "null" }
                if (code == "cancelled") {
                    activeCommandId = null
                    post { onTurnState("cancelled") }
                } else {
                    if (id != null) {
                        activeCommandId = null
                        post { onTurnState("idle") }
                    }
                    val message = payload.optString("message", "Desktop assistant báo lỗi.")
                    post { onError(message) }
                }

                if (code == "authentication_failed" || code == "device_revoked") {
                    desiredConnection = false
                    reconnectScheduled = false
                    ready = false
                    if (generation == connectionGeneration) {
                        webSocket.close(1008, code)
                    }
                    post {
                        onConnectionChanged(false, "Cần pair/cho phép lại thiết bị")
                    }
                }
            }
        }
    }

    private fun scheduleReconnect(url: String, token: String, generation: Long) {
        if (!isCurrentConnection(generation) || reconnectScheduled) return
        reconnectScheduled = true
        val shift = min(reconnectAttempt, 4)
        val delay = min(INITIAL_RECONNECT_DELAY_MS shl shift, MAX_RECONNECT_DELAY_MS)
        reconnectAttempt += 1
        post {
            onConnectionChanged(false, "Mất kết nối · thử lại sau ${delay / 1000}s")
        }
        mainHandler.postDelayed({
            if (!isCurrentConnection(generation)) {
                reconnectScheduled = false
                return@postDelayed
            }
            openWebSocket(url, token, generation)
        }, delay)
    }

    private fun isCurrentConnection(generation: Long): Boolean =
        desiredConnection && generation == connectionGeneration

    private fun helloPayload(token: String): JSONObject = JSONObject()
        .put("type", "hello")
        .put("token", token.trim())
        .put("device_id", deviceId)
        .put("device_name", deviceName)
        .put("protocol", 1)

    private fun normalizeUrl(rawUrl: String): String? {
        val trimmed = rawUrl.trim().trimEnd('/')
        if (trimmed.isEmpty()) return null
        val withScheme = when {
            trimmed.startsWith("ws://") || trimmed.startsWith("wss://") -> trimmed
            else -> "ws://$trimmed"
        }
        return runCatching { Request.Builder().url(withScheme).build().url.toString() }.getOrNull()
    }

    private fun post(block: () -> Unit) {
        mainHandler.post(block)
    }

    private fun loadOrCreateDeviceId(context: Context): String {
        val prefs = context.getSharedPreferences("voice_satellite_identity", Context.MODE_PRIVATE)
        val existing = prefs.getString("device_id", null)?.trim()
        if (!existing.isNullOrEmpty()) return existing

        val created = UUID.randomUUID().toString()
        prefs.edit().putString("device_id", created).apply()
        return created
    }
}

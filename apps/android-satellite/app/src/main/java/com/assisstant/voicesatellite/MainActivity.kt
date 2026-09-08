package com.assisstant.voicesatellite

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.weight
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp

class MainActivity : ComponentActivity() {
    companion object {
        const val EXTRA_START_VOICE = "com.assisstant.voicesatellite.START_VOICE"
        private const val CONVERSATION_FOLLOW_UP_DELAY_MS = 450L
    }

    private lateinit var satelliteClient: SatelliteClient
    private lateinit var speechController: SpeechController
    private lateinit var pairingSecretStore: PairingSecretStore
    private val conversationHandler = Handler(Looper.getMainLooper())

    private var desktopAddress by mutableStateOf("ws://192.168.1.20:8765")
    private var pairingToken by mutableStateOf("")
    private var recognitionLanguage by mutableStateOf("vi-VN")
    private var responseLanguage by mutableStateOf("vi")
    private var preferOnDevice by mutableStateOf(true)
    private var conversationModeEnabled by mutableStateOf(false)

    private var connected by mutableStateOf(false)
    private var connectionStatus by mutableStateOf("Chưa kết nối")
    private var listening by mutableStateOf(false)
    private var assistantTurnState by mutableStateOf("idle")
    private var partialText by mutableStateOf("")
    private var lastResponse by mutableStateOf("")
    private var statusMessage by mutableStateOf("")
    private var pendingVoiceActivation = false
    private var conversationActive by mutableStateOf(false)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        pairingSecretStore = PairingSecretStore(this)
        loadSettings()
        consumePairingIntent(intent)
        consumeVoiceActivationIntent(intent)

        satelliteClient = SatelliteClient(
            context = this,
            onConnectionChanged = { ready, status ->
                connected = ready
                connectionStatus = status
                if (!ready) {
                    assistantTurnState = "idle"
                    stopConversationSession(
                        cancelRecognizer = true,
                        message = if (conversationActive) {
                            "Hội thoại đã dừng vì mất kết nối desktop."
                        } else {
                            null
                        },
                    )
                } else if (pendingVoiceActivation) {
                    handlePendingVoiceActivation()
                }
            },
            onTurnState = { state ->
                assistantTurnState = if (state == "cancelled") "idle" else state
                statusMessage = when (state) {
                    "processing" -> "Desktop đang xử lý lệnh…"
                    "speaking" -> "Desktop đang trả lời…"
                    "cancelled" -> "Đã dừng Assistant."
                    "idle" -> "Sẵn sàng"
                    else -> state
                }
                if (state == "cancelled" && pendingVoiceActivation) {
                    handlePendingVoiceActivation()
                }
            },
            onResponse = { text, ttsError ->
                assistantTurnState = "idle"
                lastResponse = text
                if (ttsError == null) {
                    if (conversationActive && conversationModeEnabled) {
                        statusMessage = "Desktop đã trả lời. Chuẩn bị nghe lượt tiếp theo…"
                        scheduleConversationFollowUp()
                    } else {
                        statusMessage = "Hoàn tất"
                    }
                } else {
                    stopConversationSession(cancelRecognizer = false)
                    statusMessage = "Đã xử lý, nhưng desktop TTS gặp lỗi: $ttsError"
                }
            },
            onError = { message ->
                if (conversationActive) {
                    stopConversationSession(cancelRecognizer = true)
                }
                statusMessage = message
            },
        )

        speechController = SpeechController(
            context = this,
            onListeningChanged = { listening = it },
            onPartialText = { partialText = it },
            onFinalText = { finalText ->
                partialText = finalText
                if (!satelliteClient.sendCommand(finalText, responseLanguage)) {
                    stopConversationSession(cancelRecognizer = false)
                    statusMessage = "Chưa kết nối với desktop hoặc kết nối chưa sẵn sàng."
                } else {
                    assistantTurnState = "processing"
                    statusMessage = "Đã gửi lệnh cho desktop."
                }
            },
            onStatus = { message -> statusMessage = message },
            onError = { message ->
                if (conversationActive) {
                    stopConversationSession(cancelRecognizer = false)
                    statusMessage = "$message Hội thoại đã dừng."
                } else {
                    statusMessage = message
                }
            },
        )

        setContent {
            MaterialTheme {
                Surface(modifier = Modifier.fillMaxSize()) {
                    SatelliteScreen()
                }
            }
        }

        handlePendingVoiceActivation()
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        consumePairingIntent(intent)
        consumeVoiceActivationIntent(intent)
        if (::satelliteClient.isInitialized && ::speechController.isInitialized) {
            handlePendingVoiceActivation()
        }
    }

    override fun onDestroy() {
        conversationHandler.removeCallbacksAndMessages(null)
        speechController.destroy()
        satelliteClient.shutdown()
        super.onDestroy()
    }

    @Composable
    private fun SatelliteScreen() {
        val microphonePermission = rememberLauncherForActivityResult(
            ActivityResultContracts.RequestPermission(),
        ) { granted ->
            if (granted) {
                pendingVoiceActivation = false
                startSpeechRecognition()
            } else {
                pendingVoiceActivation = false
                stopConversationSession(cancelRecognizer = false)
                statusMessage = "Cần quyền microphone để nhận lệnh giọng nói."
            }
        }

        DisposableEffect(Unit) {
            onDispose {
                saveSettings()
            }
        }

        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Text("Assistant Voice Satellite", style = MaterialTheme.typography.headlineSmall)
            Text(
                "Điện thoại nhận giọng nói; desktop xử lý lệnh và phát câu trả lời.",
                style = MaterialTheme.typography.bodyMedium,
            )

            OutlinedTextField(
                value = desktopAddress,
                onValueChange = { desktopAddress = it },
                label = { Text("Desktop WebSocket") },
                placeholder = { Text("ws://192.168.1.20:8765") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )

            OutlinedTextField(
                value = pairingToken,
                onValueChange = { pairingToken = it },
                label = { Text("Pairing token") },
                singleLine = true,
                visualTransformation = PasswordVisualTransformation(),
                modifier = Modifier.fillMaxWidth(),
            )

            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Button(
                    onClick = {
                        saveSettings()
                        satelliteClient.connect(desktopAddress, pairingToken)
                    },
                ) {
                    Text(if (connected) "Kết nối lại" else "Kết nối")
                }
                OutlinedButton(
                    onClick = {
                        pendingVoiceActivation = false
                        stopConversationSession(cancelRecognizer = true)
                        satelliteClient.close()
                        connected = false
                        assistantTurnState = "idle"
                        connectionStatus = "Đã ngắt kết nối"
                    },
                    enabled = connected,
                ) {
                    Text("Ngắt")
                }
            }
            Text("Trạng thái: $connectionStatus")

            HorizontalDivider()

            Text("Ngôn ngữ nhận dạng", style = MaterialTheme.typography.titleMedium)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilterChip(
                    selected = recognitionLanguage == "vi-VN",
                    onClick = { recognitionLanguage = "vi-VN" },
                    label = { Text("Tiếng Việt") },
                )
                FilterChip(
                    selected = recognitionLanguage == "en-US",
                    onClick = { recognitionLanguage = "en-US" },
                    label = { Text("English") },
                )
            }

            Text("Ngôn ngữ desktop phản hồi", style = MaterialTheme.typography.titleMedium)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilterChip(
                    selected = responseLanguage == "vi",
                    onClick = { responseLanguage = "vi" },
                    label = { Text("VI") },
                )
                FilterChip(
                    selected = responseLanguage == "en",
                    onClick = { responseLanguage = "en" },
                    label = { Text("EN") },
                )
                FilterChip(
                    selected = responseLanguage == "auto",
                    onClick = { responseLanguage = "auto" },
                    label = { Text("Auto") },
                )
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text("Ưu tiên nhận dạng on-device")
                    Text(
                        "Nếu thiết bị hỗ trợ; nếu không sẽ dùng dịch vụ SpeechRecognizer hệ thống.",
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
                Switch(
                    checked = preferOnDevice,
                    onCheckedChange = { preferOnDevice = it },
                )
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text("Hội thoại liên tục")
                    Text(
                        "Sau khi desktop nói xong, điện thoại tự mở một lượt nghe tiếp theo. Không nghe khi desktop đang phát TTS.",
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
                Switch(
                    checked = conversationModeEnabled,
                    onCheckedChange = { enabled ->
                        conversationModeEnabled = enabled
                        if (!enabled && conversationActive) {
                            stopConversationSession(
                                cancelRecognizer = true,
                                message = "Đã tắt chế độ hội thoại.",
                            )
                        }
                        saveSettings()
                    },
                )
            }

            if (conversationActive) {
                Text(
                    "Phiên hội thoại đang hoạt động. Im lặng đến timeout hoặc lỗi nhận dạng sẽ kết thúc phiên thay vì tự lặp vô hạn.",
                    style = MaterialTheme.typography.bodySmall,
                )
                OutlinedButton(
                    modifier = Modifier.fillMaxWidth(),
                    onClick = {
                        stopConversationSession(
                            cancelRecognizer = true,
                            message = "Đã kết thúc hội thoại.",
                        )
                    },
                ) {
                    Text("Kết thúc hội thoại")
                }
            }

            Spacer(Modifier.height(4.dp))
            Button(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(58.dp),
                enabled = connected,
                onClick = {
                    when {
                        listening -> speechController.stop()
                        assistantTurnState in setOf("processing", "speaking") -> {
                            requestInterruptAndTalk()
                        }
                        checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED -> {
                            startSpeechRecognition()
                        }
                        else -> microphonePermission.launch(Manifest.permission.RECORD_AUDIO)
                    }
                },
            ) {
                Text(
                    when {
                        listening -> "Dừng nghe"
                        assistantTurnState in setOf("processing", "speaking") -> "Nói ngắt Assistant"
                        else -> "Nói với Assistant"
                    }
                )
            }

            OutlinedButton(
                modifier = Modifier.fillMaxWidth(),
                enabled = connected && assistantTurnState in setOf("processing", "speaking"),
                onClick = {
                    pendingVoiceActivation = false
                    stopConversationSession(cancelRecognizer = true)
                    if (satelliteClient.sendCancel()) {
                        statusMessage = "Đang yêu cầu desktop dừng…"
                    } else {
                        statusMessage = "Không thể gửi yêu cầu dừng cho desktop."
                    }
                },
            ) {
                Text("Dừng Assistant")
            }

            Text(
                "Mẹo: thêm tile “Assistant Voice” vào Quick Settings để mở thẳng chế độ nói.",
                style = MaterialTheme.typography.bodySmall,
            )

            if (partialText.isNotBlank()) {
                Text("Bạn nói", style = MaterialTheme.typography.titleSmall)
                Text(partialText, style = MaterialTheme.typography.bodyLarge)
            }

            if (statusMessage.isNotBlank()) {
                Text(statusMessage, style = MaterialTheme.typography.bodyMedium)
            }

            if (lastResponse.isNotBlank()) {
                HorizontalDivider()
                Text("Desktop trả lời", style = MaterialTheme.typography.titleSmall)
                Text(lastResponse, style = MaterialTheme.typography.bodyLarge)
            }
        }
    }

    private fun consumePairingIntent(intent: Intent?) {
        if (intent?.action != Intent.ACTION_VIEW) return

        PairingDeepLink.parse(intent.data)
            .onSuccess { pairing ->
                if (::satelliteClient.isInitialized) {
                    satelliteClient.close()
                }
                pendingVoiceActivation = false
                if (::speechController.isInitialized) {
                    stopConversationSession(cancelRecognizer = true)
                } else {
                    conversationActive = false
                    conversationHandler.removeCallbacksAndMessages(null)
                }
                connected = false
                assistantTurnState = "idle"
                connectionStatus = "Đã nhập pairing từ QR"
                desktopAddress = pairing.desktopAddress
                pairingToken = pairing.token
                saveSettings()
                statusMessage = "Đã nhập pairing từ QR. Kiểm tra địa chỉ rồi nhấn Kết nối."
            }
            .onFailure { error ->
                statusMessage = error.message ?: "Không thể đọc pairing QR."
            }
    }

    private fun consumeVoiceActivationIntent(intent: Intent?) {
        if (intent?.getBooleanExtra(EXTRA_START_VOICE, false) == true) {
            pendingVoiceActivation = true
            intent.removeExtra(EXTRA_START_VOICE)
        }
    }

    private fun handlePendingVoiceActivation() {
        if (!pendingVoiceActivation) return
        if (pairingToken.isBlank()) {
            pendingVoiceActivation = false
            statusMessage = "Chưa có pairing với desktop. Hãy quét QR trước."
            return
        }
        if (checkSelfPermission(Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED) {
            pendingVoiceActivation = false
            statusMessage = "Quick Settings cần quyền microphone đã được cấp trước. Hãy bấm Nói với Assistant một lần để cấp quyền."
            return
        }
        if (!satelliteClient.isReady()) {
            statusMessage = "Đang kết nối desktop để bắt đầu nghe…"
            satelliteClient.connect(desktopAddress, pairingToken)
            return
        }
        if (assistantTurnState in setOf("processing", "speaking")) {
            if (!satelliteClient.sendCancel()) {
                pendingVoiceActivation = false
                statusMessage = "Không thể dừng tác vụ hiện tại để bắt đầu nghe."
            } else {
                statusMessage = "Đang dừng câu trả lời hiện tại…"
            }
            return
        }
        if (assistantTurnState != "idle") {
            pendingVoiceActivation = false
            statusMessage = "Assistant hiện chưa sẵn sàng nhận lệnh mới."
            return
        }

        pendingVoiceActivation = false
        startSpeechRecognition()
    }

    private fun requestInterruptAndTalk() {
        if (checkSelfPermission(Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED) {
            statusMessage = "Cần quyền microphone trước khi có thể nói ngắt Assistant."
            return
        }
        pendingVoiceActivation = true
        conversationHandler.removeCallbacksAndMessages(null)
        speechController.cancel()
        if (satelliteClient.sendCancel()) {
            statusMessage = "Đang dừng Assistant để nghe lệnh mới…"
        } else {
            pendingVoiceActivation = false
            stopConversationSession(cancelRecognizer = false)
            statusMessage = "Không thể gửi yêu cầu dừng cho desktop."
        }
    }

    private fun startSpeechRecognition(clearLastResponse: Boolean = true) {
        conversationHandler.removeCallbacksAndMessages(null)
        if (!satelliteClient.isReady()) {
            stopConversationSession(cancelRecognizer = false)
            statusMessage = "Hãy kết nối với desktop trước."
            return
        }
        if (assistantTurnState != "idle") {
            statusMessage = "Assistant đang xử lý tác vụ khác. Hãy dừng hoặc chờ hoàn tất."
            return
        }
        if (checkSelfPermission(Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED) {
            stopConversationSession(cancelRecognizer = false)
            statusMessage = "Cần quyền microphone để nhận lệnh giọng nói."
            return
        }

        if (conversationModeEnabled) {
            conversationActive = true
        }
        saveSettings()
        if (clearLastResponse) {
            lastResponse = ""
        }
        statusMessage = if (conversationActive) {
            "Đang nghe lượt hội thoại…"
        } else if (preferOnDevice) {
            "Đang mở bộ nhận dạng giọng nói…"
        } else {
            "Đang nghe…"
        }
        speechController.start(recognitionLanguage, preferOnDevice)
    }

    private fun scheduleConversationFollowUp() {
        conversationHandler.removeCallbacksAndMessages(null)
        if (!conversationActive || !conversationModeEnabled) return

        conversationHandler.postDelayed({
            if (!conversationActive || !conversationModeEnabled) return@postDelayed
            if (!satelliteClient.isReady()) {
                stopConversationSession(
                    cancelRecognizer = false,
                    message = "Hội thoại đã dừng vì desktop không còn kết nối.",
                )
                return@postDelayed
            }
            if (assistantTurnState != "idle" || listening) {
                stopConversationSession(
                    cancelRecognizer = false,
                    message = "Hội thoại đã dừng vì Assistant chưa sẵn sàng cho lượt tiếp theo.",
                )
                return@postDelayed
            }
            if (checkSelfPermission(Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED) {
                stopConversationSession(
                    cancelRecognizer = false,
                    message = "Hội thoại đã dừng vì ứng dụng không còn quyền microphone.",
                )
                return@postDelayed
            }

            partialText = ""
            statusMessage = "Đang nghe lượt tiếp theo…"
            startSpeechRecognition(clearLastResponse = false)
        }, CONVERSATION_FOLLOW_UP_DELAY_MS)
    }

    private fun stopConversationSession(
        cancelRecognizer: Boolean,
        message: String? = null,
    ) {
        conversationHandler.removeCallbacksAndMessages(null)
        conversationActive = false
        if (cancelRecognizer && ::speechController.isInitialized) {
            speechController.cancel()
        }
        if (message != null) {
            statusMessage = message
        }
    }

    private fun loadSettings() {
        val prefs = getSharedPreferences("voice_satellite", MODE_PRIVATE)
        desktopAddress = prefs.getString("desktop_address", desktopAddress) ?: desktopAddress
        recognitionLanguage = prefs.getString("recognition_language", "vi-VN") ?: "vi-VN"
        responseLanguage = prefs.getString("response_language", "vi") ?: "vi"
        preferOnDevice = prefs.getBoolean("prefer_on_device", true)
        conversationModeEnabled = prefs.getBoolean("conversation_mode", false)

        val legacyToken = prefs.getString("pairing_token", null)?.trim().orEmpty()
        val secureToken = pairingSecretStore.loadToken()
        secureToken
            .onSuccess { token ->
                if (!token.isNullOrBlank()) {
                    pairingToken = token
                    if (legacyToken.isNotEmpty()) {
                        prefs.edit().remove("pairing_token").apply()
                    }
                } else if (legacyToken.isNotEmpty()) {
                    val migration = pairingSecretStore.saveToken(legacyToken)
                    if (migration.isSuccess) {
                        pairingToken = legacyToken
                        prefs.edit().remove("pairing_token").apply()
                    } else {
                        pairingToken = legacyToken
                        statusMessage = "Android Keystore chưa lưu được pairing token cũ; token plaintext tạm thời được giữ để tránh mất pairing."
                    }
                }
            }
            .onFailure {
                pairingToken = legacyToken
                statusMessage = "Không thể đọc pairing token từ Android Keystore. Hãy quét lại QR nếu kết nối không còn hoạt động."
            }
    }

    private fun saveSettings() {
        val prefs = getSharedPreferences("voice_satellite", MODE_PRIVATE)
        prefs.edit()
            .putString("desktop_address", desktopAddress.trim())
            .putString("recognition_language", recognitionLanguage)
            .putString("response_language", responseLanguage)
            .putBoolean("prefer_on_device", preferOnDevice)
            .putBoolean("conversation_mode", conversationModeEnabled)
            .apply()

        pairingSecretStore.saveToken(pairingToken)
            .onSuccess {
                prefs.edit().remove("pairing_token").apply()
            }
            .onFailure {
                if (pairingToken.isNotBlank()) {
                    statusMessage = "Không thể lưu pairing token vào Android Keystore; token mới chỉ tồn tại trong phiên hiện tại."
                }
            }
    }
}
